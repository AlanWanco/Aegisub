// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#include "gpui_bridge.h"

#include "ass_dialogue.h"
#include "ass_file.h"
#include "async_video_provider.h"
#include "command/command.h"
#include "compat.h"
#include "include/aegisub/context.h"
#include "main.h"
#include "project.h"
#include "selection_controller.h"
#include "subs_controller.h"
#include "video_controller.h"
#include "video_frame.h"

#include <libaegisub/cajun/reader.h>
#include <libaegisub/cajun/visitor.h>
#include <libaegisub/log.h>

#include <wx/base64.h>
#include <wx/filefn.h>
#include <wx/image.h>
#include <wx/mstream.h>
#include <wx/sckaddr.h>
#include <wx/socket.h>
#include <wx/timer.h>

#include <algorithm>
#include <array>
#include <cstdint>
#include <iomanip>
#include <map>
#include <random>
#include <sstream>
#include <string_view>
#include <vector>

namespace {
constexpr size_t max_message_size = 2 * 1024 * 1024;

std::string make_token() {
	std::array<unsigned char, 32> bytes{};
	std::random_device random;
	for (auto& byte : bytes)
		byte = static_cast<unsigned char>(random());

	std::ostringstream out;
	out << std::hex << std::setfill('0');
	for (auto byte : bytes)
		out << std::setw(2) << static_cast<unsigned>(byte);
	return out.str();
}

std::string quote_json(std::string_view value) {
	std::ostringstream out;
	out << '"';
	for (unsigned char ch : value) {
		switch (ch) {
			case '"': out << "\\\""; break;
			case '\\': out << "\\\\"; break;
			case '\b': out << "\\b"; break;
			case '\f': out << "\\f"; break;
			case '\n': out << "\\n"; break;
			case '\r': out << "\\r"; break;
			case '\t': out << "\\t"; break;
			default:
				if (ch < 0x20)
					out << "\\u00" << std::hex << std::setw(2) << static_cast<unsigned>(ch) << std::dec;
				else
					out << static_cast<char>(ch);
		}
	}
	out << '"';
	return out.str();
}

std::string to_base64_png(VideoFrame const& frame) {
	wxImage image = GetImage(frame);
	wxMemoryOutputStream stream;
	if (!image.SaveFile(stream, wxBITMAP_TYPE_PNG))
		return {};
	std::vector<unsigned char> bytes(stream.GetSize());
	stream.CopyTo(bytes.data(), bytes.size());
	return std::string(wxBase64Encode(bytes.data(), bytes.size()).utf8_str());
}

struct JsonStringCheck final : json::ConstVisitor {
	bool is_string = false;
	void Visit(const json::Array&) override { }
	void Visit(const json::Object&) override { }
	void Visit(int64_t) override { }
	void Visit(double) override { }
	void Visit(const json::String&) override { is_string = true; }
	void Visit(bool) override { }
	void Visit(const json::Null&) override { }
};

bool is_json_string(json::UnknownElement const& element) {
	JsonStringCheck check;
	element.Accept(check);
	return check.is_string;
}
}

struct GpuiBridge::Impl final : wxEvtHandler {
	struct Client {
		std::string request;
		std::string response;
		size_t response_offset = 0;
		bool responding = false;
		bool waiting_for_frame = false;
		bool awaiting_current = false;
		int requested_frame = -1;
		uint64_t request_id = 0;
		uint64_t video_epoch = 0;
		AsyncVideoProvider *provider = nullptr;
	};

	agi::Context *context;
	std::unique_ptr<wxSocketInitializer> sockets;
	wxSocketServer *server = nullptr;
	std::map<wxSocketBase *, Client> clients;
	wxTimer frame_poll;
	unsigned short port = 0;
	uint64_t video_epoch = 0;
	AsyncVideoProvider *observed_provider = nullptr;
	std::string token = make_token();

	explicit Impl(agi::Context *context) : context(context), frame_poll(this, wxID_ANY) {
		Bind(wxEVT_SOCKET, &Impl::OnSocket, this);
		Bind(wxEVT_TIMER, &Impl::OnFramePoll, this, frame_poll.GetId());
	}

	~Impl() {
		Stop();
		Unbind(wxEVT_SOCKET, &Impl::OnSocket, this);
		Unbind(wxEVT_TIMER, &Impl::OnFramePoll, this, frame_poll.GetId());
	}

	void CloseClient(wxSocketBase *socket) {
		auto it = clients.find(socket);
		if (it == clients.end()) return;
		socket->Notify(false);
		socket->Close();
		socket->Destroy();
		clients.erase(it);
	}

	void Stop() {
		frame_poll.Stop();
		for (auto it = clients.begin(); it != clients.end();) {
			auto socket = it->first;
			++it;
			CloseClient(socket);
		}
		if (server) {
			server->Notify(false);
			server->Close();
			server->Destroy();
			server = nullptr;
		}
		if (context && context->videoController)
			context->videoController->Unbind(EVT_FRAME_READY, &Impl::OnFrameReady, this);
		observed_provider = nullptr;
		++video_epoch;
		port = 0;
		sockets.reset();
	}

	void OnFramePoll(wxTimerEvent&) {
		auto provider_now = context->project->VideoProvider();
		if (provider_now != observed_provider) {
			observed_provider = provider_now;
			++video_epoch;
		}
		if (!provider_now) {
			std::vector<wxSocketBase *> pending;
			for (auto const& [socket, client] : clients)
				if (client.waiting_for_frame) pending.push_back(socket);
			for (auto socket : pending) {
				auto it = clients.find(socket);
				if (it != clients.end()) {
					it->second.waiting_for_frame = false;
					BeginResponse(socket, it->second, "{\"ok\":true,\"video\":false,\"frame\":-1,\"request\":" + std::to_string(it->second.request_id) + ",\"image\":null}");
				}
			}
			return;
		}

		int const frame_number = context->videoController->GetFrameN();
		for (auto& [socket, client] : clients) {
			if (!client.waiting_for_frame || (client.requested_frame == frame_number && client.video_epoch == video_epoch && client.provider == provider_now)) continue;
			client.requested_frame = frame_number;
			client.video_epoch = video_epoch;
			client.provider = provider_now;
			client.awaiting_current = true;
			if (auto provider = context->project->VideoProvider())
				provider->RequestFrame(frame_number, context->videoController->TimeAtFrame(frame_number));
		}
	}

	void OnSocket(wxSocketEvent& event) {
		wxSocketBase *socket = event.GetSocket();
		if (socket == server) {
			if (event.GetSocketEvent() != wxSOCKET_CONNECTION) return;
			wxSocketBase *client = server->Accept(false);
			if (!client) return;
			client->SetFlags(wxSOCKET_NOWAIT);
			client->SetTimeout(5);
			client->SetEventHandler(*this, wxID_ANY);
			client->SetNotify(wxSOCKET_INPUT_FLAG | wxSOCKET_LOST_FLAG);
			client->Notify(true);
			clients.emplace(client, Client{});
			return;
		}

		auto it = clients.find(socket);
		if (it == clients.end()) return;
		if (event.GetSocketEvent() == wxSOCKET_LOST || socket->Error()) {
			CloseClient(socket);
			return;
		}
		if (event.GetSocketEvent() == wxSOCKET_INPUT) {
			ReadRequest(socket, it->second);
			return;
		}
		if (event.GetSocketEvent() == wxSOCKET_OUTPUT)
			WriteResponse(socket, it->second);
	}

	void ReadRequest(wxSocketBase *socket, Client& client) {
		char buffer[4096];
		while (socket->IsData()) {
			socket->Read(buffer, sizeof buffer);
			size_t count = socket->LastCount();
			if (!count) break;
			client.request.append(buffer, count);
			if (client.request.size() > max_message_size) {
				BeginResponse(socket, client, R"({"ok":false,"error":"request too large"})");
				return;
			}
			auto newline = client.request.find('\n');
			if (newline != std::string::npos) {
				client.request.resize(newline);
				try {
					Handle(client.request, socket, client);
				}
				catch (std::exception const& error) {
					BeginResponse(socket, client, "{\"ok\":false,\"error\":" + quote_json(error.what()) + "}");
				}
				catch (...) {
					BeginResponse(socket, client, R"({"ok":false,"error":"unknown bridge error"})");
				}
				return;
			}
		}
	}

	void BeginResponse(wxSocketBase *socket, Client& client, std::string response) {
		client.response = std::move(response);
		client.response.push_back('\n');
		client.responding = true;
		socket->SetNotify(wxSOCKET_OUTPUT_FLAG | wxSOCKET_LOST_FLAG);
		socket->Notify(true);
		WriteResponse(socket, client);
	}

	void OnFrameReady(FrameReadyEvent& event) {
		auto current_provider = context->project->VideoProvider();
		if (current_provider != observed_provider) {
			observed_provider = current_provider;
			++video_epoch;
		}
		std::vector<wxSocketBase *> ready;
		int const current_frame = context->videoController->GetFrameN();
		for (auto const& [socket, client] : clients) {
			if (client.waiting_for_frame && client.video_epoch == video_epoch && client.provider == context->project->VideoProvider() && client.requested_frame == current_frame && static_cast<int>(event.time) == context->videoController->TimeAtFrame(current_frame))
				ready.push_back(socket);
		}
		for (auto socket : ready) {
			auto it = clients.find(socket);
			if (it == clients.end()) continue;
			it->second.waiting_for_frame = false;
			it->second.awaiting_current = false;
			it->second.provider = nullptr;
			std::string encoded = to_base64_png(*event.frame);
			if (encoded.empty()) {
				BeginResponse(socket, it->second, R"({"ok":false,"error":"could not encode preview frame"})");
				continue;
			}
			std::ostringstream out;
			out << "{\"ok\":true,\"video\":true,\"frame\":" << it->second.requested_frame
			    << ",\"request\":" << it->second.request_id
			    << ",\"width\":" << event.frame->width << ",\"height\":" << event.frame->height
			    << ",\"image\":" << quote_json(encoded) << '}';
			BeginResponse(socket, it->second, out.str());
		}
		event.Skip();
	}

	void WriteResponse(wxSocketBase *socket, Client& client) {
		while (client.response_offset < client.response.size()) {
			size_t count = std::min<size_t>(65536, client.response.size() - client.response_offset);
			socket->Write(client.response.data() + client.response_offset, count);
			size_t written = socket->LastCount();
			client.response_offset += written;
			if (socket->Error()) {
				if (socket->LastError() == wxSOCKET_WOULDBLOCK) return;
				CloseClient(socket);
				return;
			}
			if (!written) return;
		}
		CloseClient(socket);
	}

	void Handle(std::string const& request, wxSocketBase *socket, Client& client) {
		std::istringstream stream(request);
		json::UnknownElement parsed;
		json::Reader::Read(parsed, stream);
		auto const& object = static_cast<json::Object const&>(parsed);
		auto token_it = object.find("token");
		if (token_it == object.end() || static_cast<std::string const&>(token_it->second) != token) {
			BeginResponse(socket, client, R"({"ok":false,"error":"unauthorized"})");
			return;
		}

		auto op_it = object.find("op");
		if (op_it == object.end()) {
			BeginResponse(socket, client, R"({"ok":false,"error":"missing operation"})");
			return;
		}
		std::string const& op = op_it->second;

		if (op == "snapshot") { BeginResponse(socket, client, Snapshot()); return; }
		if (op == "frame") { QueueFrame(object, socket, client); return; }
		if (op == "select") { BeginResponse(socket, client, Select(object)); return; }
		if (op == "edit") { BeginResponse(socket, client, Edit(object)); return; }
		if (op == "seek") { BeginResponse(socket, client, Seek(object)); return; }
		if (op == "command") { BeginResponse(socket, client, Command(object)); return; }
		if (op == "open") { BeginResponse(socket, client, Open(object)); return; }
		if (op == "save_as") { BeginResponse(socket, client, SaveAs(object)); return; }
		BeginResponse(socket, client, R"({"ok":false,"error":"unsupported operation"})");
	}

	std::string Snapshot() {
		auto current_provider = context->project->VideoProvider();
		if (current_provider != observed_provider) {
			observed_provider = current_provider;
			++video_epoch;
		}
		std::ostringstream out;
		out << "{\"ok\":true,\"file\":"
		    << quote_json(context->subsController->Filename().string())
		    << ",\"modified\":" << (context->subsController->IsModified() ? "true" : "false")
		    << ",\"video\":" << (context->project->VideoProvider() ? "true" : "false")
		    << ",\"playing\":" << (context->videoController->IsPlaying() ? "true" : "false")
		    << ",\"frame\":" << context->videoController->GetFrameN()
		    << ",\"video_epoch\":" << video_epoch
		    << ",\"frame_count\":";
		if (auto provider = context->project->VideoProvider())
			out << provider->GetFrameCount();
		else
			out << 0;
		out << ",\"fps\":";
		if (auto provider = context->project->VideoProvider())
			out << provider->GetFPS().FPS();
		else
			out << "null";
		out << ",\"active\":";
		auto active = context->selectionController->GetActiveLine();
		out << (active ? active->Id : -1) << ",\"lines\":[";
		bool first = true;
		for (auto const& line : context->ass->Events) {
			if (!first) out << ',';
			first = false;
			out << "{\"id\":" << line.Id
			    << ",\"row\":" << line.Row
			    << ",\"start\":" << static_cast<int>(line.Start)
			    << ",\"end\":" << static_cast<int>(line.End)
			    << ",\"layer\":" << line.Layer
			    << ",\"comment\":" << (line.Comment ? "true" : "false")
			    << ",\"style\":" << quote_json(line.Style.get())
			    << ",\"actor\":" << quote_json(line.Actor.get())
			    << ",\"text\":" << quote_json(line.Text.get()) << '}';
		}
		out << "]}";
		return out.str();
	}

	void QueueFrame(json::Object const& object, wxSocketBase *socket, Client& client) {
		auto provider = context->project->VideoProvider();
		if (provider != observed_provider) {
			observed_provider = provider;
			++video_epoch;
		}
		if (!provider) {
			uint64_t request_id = 0;
			if (auto request_it = object.find("request"); request_it != object.end())
				request_id = static_cast<uint64_t>(static_cast<json::Integer const&>(request_it->second));
			BeginResponse(socket, client, "{\"ok\":true,\"video\":false,\"frame\":-1,\"request\":" + std::to_string(request_id) + ",\"image\":null}");
			return;
		}
		auto frame_it = object.find("frame");
		auto request_it = object.find("request");
		int frame_number = frame_it == object.end() ? context->videoController->GetFrameN() : static_cast<int>(static_cast<json::Integer const&>(frame_it->second));
		frame_number = std::clamp(frame_number, 0, provider->GetFrameCount() - 1);
		uint64_t request_id = request_it == object.end() ? 0 : static_cast<uint64_t>(static_cast<json::Integer const&>(request_it->second));
		client.waiting_for_frame = true;
		client.awaiting_current = frame_number != context->videoController->GetFrameN();
		client.requested_frame = frame_number;
		client.request_id = request_id;
		client.video_epoch = video_epoch;
		client.provider = provider;
		provider->RequestFrame(frame_number, context->videoController->TimeAtFrame(frame_number));
	}

	std::string Select(json::Object const& object) {
		auto id_it = object.find("id");
		if (id_it == object.end())
			return R"({"ok":false,"error":"missing line id"})";
		int id = static_cast<int>(static_cast<json::Integer const&>(id_it->second));
		for (auto& line : context->ass->Events) {
			if (line.Id != id) continue;
			context->selectionController->SetSelectionAndActive({&line}, &line);
			return R"({"ok":true})";
		}
		return R"({"ok":false,"error":"line no longer exists"})";
	}

	std::string Edit(json::Object const& object) {
		auto id_it = object.find("id");
		auto text_it = object.find("text");
		if (id_it == object.end() || text_it == object.end())
			return R"({"ok":false,"error":"missing line id or text"})";
		int id = static_cast<int>(static_cast<json::Integer const&>(id_it->second));
		std::string const& text = text_it->second;
		for (auto& line : context->ass->Events) {
			if (line.Id != id) continue;
			if (line.Text == text)
				return R"({"ok":true,"unchanged":true})";
			line.Text = text;
			context->selectionController->SetSelectionAndActive({&line}, &line);
			context->ass->Commit(_("GPUI subtitle edit"), AssFile::COMMIT_DIAG_TEXT, -1, &line);
			return R"({"ok":true})";
		}
		return R"({"ok":false,"error":"line no longer exists"})";
	}

	std::string Seek(json::Object const& object) {
		auto frame_it = object.find("frame");
		if (frame_it == object.end())
			return R"({"ok":false,"error":"missing frame"})";
		context->videoController->JumpToFrame(static_cast<int>(static_cast<json::Integer const&>(frame_it->second)));
		return R"({"ok":true})";
	}

	std::string Open(json::Object const& object) {
		auto kind_it = object.find("kind");
		auto path_it = object.find("path");
		if (kind_it == object.end() || path_it == object.end())
			return R"({"ok":false,"error":"missing file kind or path"})";
		if (!is_json_string(path_it->second) || !is_json_string(kind_it->second))
			return R"({"ok":false,"error":"file kind and path must be strings"})";
		std::string const& kind = kind_it->second;
		agi::fs::path path(static_cast<std::string const&>(path_it->second));
		if (kind == "subtitle") {
			if (context->subsController->TryToClose() == wxCANCEL)
				return R"({"ok":false,"error":"opening subtitles was cancelled"})";
			context->project->LoadSubtitles(path);
			return R"({"ok":true})";
		}
		if (kind == "video") {
			context->project->LoadVideo(path);
			return R"({"ok":true})";
		}
		return R"({"ok":false,"error":"unsupported file kind"})";
	}

	std::string SaveAs(json::Object const& object) {
		auto path_it = object.find("path");
		if (path_it == object.end())
			return R"({"ok":false,"error":"missing save path"})";
		try {
			if (!is_json_string(path_it->second))
				return R"({"ok":false,"error":"save path must be a string"})";
			std::string const& path = path_it->second;
			if (wxFileExists(wxString::FromUTF8(path)) && wxMessageBox(
				wxString::Format(_("The file %s already exists. Replace it?"), wxString::FromUTF8(path)),
				_("Confirm Save As"), wxYES_NO | wxNO_DEFAULT | wxICON_WARNING | wxCENTER, nullptr) != wxYES)
				return R"({"ok":false,"error":"save was cancelled"})";
			context->subsController->Save(agi::fs::path(path));
			return R"({"ok":true})";
		}
		catch (agi::Exception const& error) {
			return "{\"ok\":false,\"error\":" + quote_json(error.GetMessage()) + "}";
		}
		catch (std::exception const& error) {
			return "{\"ok\":false,\"error\":" + quote_json(error.what()) + "}";
		}
	}

	std::string Command(json::Object const& object) {
		auto name_it = object.find("name");
		if (name_it == object.end())
			return R"({"ok":false,"error":"missing command name"})";
		std::string const& name = name_it->second;
		static const std::vector<std::string> allowed = {
			"subtitle/open", "subtitle/save", "subtitle/save/as", "subtitle/new",
			"video/close", "video/play", "video/stop",
			"edit/undo", "edit/redo", "video/tool/drag", "video/tool/clip",
			"video/tool/scale", "video/tool/rotate/z", "video/tool/rotate/xy",
			"video/tool/vector_clip"
		};
		if (std::find(allowed.begin(), allowed.end(), name) == allowed.end())
			return R"({"ok":false,"error":"command is not available in the GPUI prototype"})";
		if (name == "video/play") {
			context->videoController->Play();
			return R"({"ok":true})";
		}
		if (name == "video/stop") {
			context->videoController->Stop();
			return R"({"ok":true})";
		}
		cmd::call(name, context);
		return R"({"ok":true})";
	}
};

GpuiBridge::GpuiBridge(agi::Context *context) : impl(std::make_unique<Impl>(context)) { }
GpuiBridge::~GpuiBridge() = default;

bool GpuiBridge::Start() {
	if (impl->server) return true;
	impl->sockets = std::make_unique<wxSocketInitializer>();
	wxIPV4address address;
	address.LocalHost();
	address.Service(static_cast<unsigned short>(0));
	auto server = std::make_unique<wxSocketServer>(address, wxSOCKET_NOWAIT | wxSOCKET_REUSEADDR);
	if (!server->IsOk()) {
		LOG_E("gpui/bridge") << "Could not bind the local GPUI bridge socket";
		impl->sockets.reset();
		return false;
	}
	wxIPV4address bound;
	if (!server->GetLocal(bound) || bound.Service() == 0) {
		LOG_E("gpui/bridge") << "Could not read the local GPUI bridge port";
		server->Destroy();
		impl->sockets.reset();
		return false;
	}
	impl->port = bound.Service();
	impl->context->videoController->Bind(EVT_FRAME_READY, &GpuiBridge::Impl::OnFrameReady, impl.get());
	impl->frame_poll.Start(40);
	impl->server = server.release();
	impl->server->SetEventHandler(*impl, wxID_ANY);
	impl->server->SetNotify(wxSOCKET_CONNECTION_FLAG | wxSOCKET_LOST_FLAG);
	impl->server->Notify(true);
	return true;
}

void GpuiBridge::Stop() { impl->Stop(); }
bool GpuiBridge::IsRunning() const { return impl->server != nullptr; }
std::string GpuiBridge::Endpoint() const { return "127.0.0.1:" + std::to_string(impl->port); }
std::string GpuiBridge::Token() const { return impl->token; }
