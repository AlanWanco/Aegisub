// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#include "gpui_color.h"

#include "command/edit.h"
#include "include/aegisub/context.h"

#include <libaegisub/cajun/writer.h>
#include <libaegisub/color.h>

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
	}
	else if (action == "apply")
		cmd::ApplyColorPick(context, agi::Color(static_cast<std::string const&>(request.at("color"))));
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
