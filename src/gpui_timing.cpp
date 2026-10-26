// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#include "gpui_timing.h"

#include "include/aegisub/context.h"
#include "ass_file.h"
#include "options.h"
#include "project.h"
#include "shift_times.h"
#include "timing_processor.h"

#include <libaegisub/cajun/writer.h>

#include <algorithm>
#include <sstream>
#include <stdexcept>

namespace gpui {
std::string ShiftTimesPanel(agi::Context *context, json::Object const& request) {
	std::string const& action = request.at("action");
	json::Object result;
	if (action == "apply") {
		auto integer = [&](char const* field, int upper) {
			int64_t const value = static_cast<json::Integer const&>(request.at(field));
			if (value < 0 || value > upper) throw std::invalid_argument("invalid shift settings");
			return static_cast<int>(value);
		};
		ShiftTimesSettings settings;
		settings.by_time = static_cast<json::Boolean const&>(request.at("by_time"));
		settings.backward = static_cast<json::Boolean const&>(request.at("backward"));
		settings.amount = integer("amount", 35999990);
		settings.mode = integer("mode", 2);
		settings.fields = integer("fields", 2);
		result["changed"] = static_cast<int64_t>(ShiftTimes(context, settings));
	}
	else if (action == "clear_history") ClearShiftTimesHistory();
	else if (action != "state") throw std::invalid_argument("unknown shift times action");
	result["ok"] = true;
	result["by_time"] = OPT_GET("Tool/Shift Times/ByTime")->GetBool();
	result["backward"] = OPT_GET("Tool/Shift Times/Direction")->GetBool();
	result["time"] = OPT_GET("Tool/Shift Times/Time")->GetInt();
	result["frames"] = OPT_GET("Tool/Shift Times/Frames")->GetInt();
	result["fields"] = OPT_GET("Tool/Shift Times/Type")->GetInt();
	result["mode"] = OPT_GET("Tool/Shift Times/Affect")->GetInt();
	result["can_frames"] = context->project->Timecodes().IsLoaded();
	result["history"] = LoadShiftTimesHistory();
	std::ostringstream stream;
	agi::JsonWriter::Write(result, stream);
	auto response = stream.str();
	response.erase(std::remove(response.begin(), response.end(), '\n'), response.end());
	return response;
}

std::string TimingProcessor(agi::Context *context, json::Object const& request) {
	std::string const& action = request.at("action");
	struct IntegerOption { char const* name; char const* option; int TimingProcessorSettings::*member; };
	IntegerOption const integers[] = {
		{"lead_in", "Tool/Timing Post Processor/Lead/IN", &TimingProcessorSettings::lead_in},
		{"lead_out", "Tool/Timing Post Processor/Lead/OUT", &TimingProcessorSettings::lead_out},
		{"before_start", "Tool/Timing Post Processor/Threshold/Key Start Before", &TimingProcessorSettings::before_start},
		{"after_start", "Tool/Timing Post Processor/Threshold/Key Start After", &TimingProcessorSettings::after_start},
		{"before_end", "Tool/Timing Post Processor/Threshold/Key End Before", &TimingProcessorSettings::before_end},
		{"after_end", "Tool/Timing Post Processor/Threshold/Key End After", &TimingProcessorSettings::after_end},
		{"adjacent_gap", "Tool/Timing Post Processor/Threshold/Adjacent Gap", &TimingProcessorSettings::adjacent_gap},
		{"adjacent_overlap", "Tool/Timing Post Processor/Threshold/Adjacent Overlap", &TimingProcessorSettings::adjacent_overlap}
	};
	struct BooleanOption { char const* name; char const* option; bool TimingProcessorSettings::*member; };
	BooleanOption const booleans[] = {
		{"has_lead_in", "Tool/Timing Post Processor/Enable/Lead/IN", &TimingProcessorSettings::has_lead_in},
		{"has_lead_out", "Tool/Timing Post Processor/Enable/Lead/OUT", &TimingProcessorSettings::has_lead_out},
		{"adjacent", "Tool/Timing Post Processor/Enable/Adjacent", &TimingProcessorSettings::adjacent},
		{"keyframes", "Tool/Timing Post Processor/Enable/Keyframe", &TimingProcessorSettings::keyframes},
		{"only_selection", "Tool/Timing Post Processor/Only Selection", &TimingProcessorSettings::only_selection}
	};
	json::Object result;
	bool const can_keys = !context->project->Keyframes().empty() && context->project->Timecodes().IsLoaded();
	if (action == "apply") {
		TimingProcessorSettings settings;
		for (auto const& entry : integers) {
			int64_t const value = static_cast<json::Integer const&>(request.at(entry.name));
			if (value < 0 || value > 35999990) throw std::invalid_argument("timing threshold is outside the permitted range");
			settings.*entry.member = static_cast<int>(value);
		}
		for (auto const& entry : booleans) settings.*entry.member = static_cast<json::Boolean const&>(request.at(entry.name));
		int64_t const bias = static_cast<json::Integer const&>(request.at("adjacent_bias"));
		if (bias < 0 || bias > 100) throw std::invalid_argument("adjacent bias must be between 0 and 100");
		settings.adjacent_bias = bias / 100.;
		for (auto const& style : static_cast<json::Array const&>(request.at("styles"))) settings.styles.insert(static_cast<std::string const&>(style));
		result["changed"] = static_cast<int64_t>(ProcessTiming(context, settings));
		for (auto const& entry : integers) OPT_SET(entry.option)->SetInt(settings.*entry.member);
		for (auto const& entry : booleans) {
			if (entry.member != &TimingProcessorSettings::keyframes || can_keys) OPT_SET(entry.option)->SetBool(settings.*entry.member);
		}
		OPT_SET("Tool/Timing Post Processor/Adjacent Bias")->SetDouble(settings.adjacent_bias);
	}
	else if (action != "state") throw std::invalid_argument("unknown timing processor action");
	result["ok"] = true;
	for (auto const& entry : integers) result[entry.name] = OPT_GET(entry.option)->GetInt();
	for (auto const& entry : booleans) result[entry.name] = OPT_GET(entry.option)->GetBool();
	result["adjacent_bias"] = static_cast<int>(OPT_GET("Tool/Timing Post Processor/Adjacent Bias")->GetDouble() * 100 + .5);
	json::Array styles;
	for (auto const& style : context->ass->GetStyles()) styles.emplace_back(style);
	result["styles"] = std::move(styles);
	result["can_keyframes"] = can_keys;
	result["keyframes"] = can_keys && OPT_GET("Tool/Timing Post Processor/Enable/Keyframe")->GetBool();
	std::ostringstream stream;
	agi::JsonWriter::Write(result, stream);
	auto response = stream.str();
	response.erase(std::remove(response.begin(), response.end(), '\n'), response.end());
	return response;
}

}
