// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <libaegisub/cajun/elements.h>
#include <cstdint>
#include <string>
#include <vector>

#include "autosave.h"

namespace agi { struct Context; }

namespace gpui {
std::string FileTools(json::Object const& request);
std::string SaveMedia(agi::Context *context, json::Object const& request);
std::string DocumentDetails(agi::Context *context);
std::string EditScriptInfo(agi::Context *context, json::Object const& request);
std::string EditStyle(agi::Context *context, json::Object const& request);
std::string StyleLibrary(agi::Context *context, json::Object const& request);
std::string Styling(agi::Context *context, json::Object const& request);
std::string Resample(agi::Context *context, json::Object const& request);
std::string Attachments(agi::Context *context, json::Object const& request, uint64_t const& revision);
}
