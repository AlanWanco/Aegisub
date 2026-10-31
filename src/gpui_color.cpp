// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#include "gpui_color.h"

#include "command/edit.h"
#include "include/aegisub/context.h"
#include "options.h"

#include <libaegisub/cajun/writer.h>
#include <libaegisub/color.h>
#include <libaegisub/option.h>

#include <algorithm>
#include <sstream>
#include <stdexcept>

namespace gpui {
std::string ColorPanel(agi::Context *context, json::Object const& request) {
	std::string const& action = request.at("action");
	json::Object result;
	if (action == "query") {
		std::string const& kind = request.at("kind");
		auto color = cmd::BeginColorPick(context, kind);
		if (!color)
			throw std::runtime_error("select a line before picking a colour");
		result["color"] = color->GetHexFormatted(true);
		// The wx picker writes the same option, so both dialogs share one history.
		json::Array recent;
		for (auto const& entry : OPT_GET("Tool/Colour Picker/Recent Colours")->GetListColor())
			recent.emplace_back(entry.GetHexFormatted(true));
		result["recent"] = std::move(recent);
	}
	else if (action == "apply") {
		agi::Color const picked(static_cast<std::string const&>(request.at("color")));
		cmd::ApplyColorPick(context, picked);
		auto recent = OPT_GET("Tool/Colour Picker/Recent Colours")->GetListColor();
		recent.erase(std::remove_if(recent.begin(), recent.end(), [&](agi::Color const& entry) {
			return entry.r == picked.r && entry.g == picked.g && entry.b == picked.b;
		}), recent.end());
		recent.insert(recent.begin(), picked);
		if (recent.size() > 16)
			recent.resize(16);
		OPT_SET("Tool/Colour Picker/Recent Colours")->SetListColor(recent);
	}
	else if (action == "cancel")
		cmd::CancelColorPick(context);
	else
		throw std::invalid_argument("unknown colour action");

	result["ok"] = true;
	std::ostringstream stream;
	agi::JsonWriter::Write(result, stream);
	auto response = stream.str();
	response.erase(std::remove(response.begin(), response.end(), '\n'), response.end());
	return response;
}
}
