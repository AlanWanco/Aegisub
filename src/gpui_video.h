// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <libaegisub/cajun/elements.h>
#include <string>

namespace agi { struct Context; }
namespace gpui {
std::string VideoSettings(agi::Context *context, json::Object const& request);
}
