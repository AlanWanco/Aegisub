// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <libaegisub/cajun/elements.h>
#include <string>

namespace agi { struct Context; }
namespace gpui {
/// Dialogs the front end draws itself: about, log, language, updates and the
/// font picker. Each request carries an `action`.
std::string Dialog(agi::Context *context, json::Object const& request);
}
