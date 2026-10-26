// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <libaegisub/cajun/elements.h>
#include <string>

namespace gpui {
std::string Appearance();
std::string Preferences(json::Object const& request);
std::string Hotkeys(json::Object const& request);
}
