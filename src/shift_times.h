// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <libaegisub/cajun/elements.h>
#include <cstddef>

namespace agi { struct Context; }

struct ShiftTimesSettings {
	bool by_time = true;
	bool backward = false;
	int amount = 0;
	int mode = 0;   // all, selected, selection onward
	int fields = 0; // start and end, start only, end only
};

json::Array LoadShiftTimesHistory();
void ClearShiftTimesHistory();
/// Shift the active document through its shared undo stack, and record the existing history format.
std::size_t ShiftTimes(agi::Context *context, ShiftTimesSettings const& settings);
