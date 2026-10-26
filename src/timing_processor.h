// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <cstddef>
#include <set>
#include <string>

namespace agi { struct Context; }

struct TimingProcessorSettings {
	std::set<std::string> styles;
	bool only_selection = false;
	bool has_lead_in = false;
	bool has_lead_out = false;
	bool adjacent = false;
	bool keyframes = false;
	int lead_in = 0;
	int lead_out = 0;
	int before_start = 0;
	int after_start = 0;
	int before_end = 0;
	int after_end = 0;
	int adjacent_gap = 0;
	int adjacent_overlap = 0;
	double adjacent_bias = 0.5;
};

/// Existing lead, adjacency and keyframe rules, independent of the frontend.
std::size_t ProcessTiming(agi::Context *context, TimingProcessorSettings const& settings);
