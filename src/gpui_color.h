// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <libaegisub/cajun/elements.h>
#include <string>

namespace agi { struct Context; }
namespace gpui {
/// Picks an override colour (\c, \2c, \3c, \4c) for the front end's own
/// colour dialog. Query first, then apply or cancel.
std::string ColorPanel(agi::Context *context, json::Object const& request);
}
