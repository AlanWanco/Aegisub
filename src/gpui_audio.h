// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <libaegisub/cajun/elements.h>
#include <memory>
#include <string>

namespace agi { struct Context; }
namespace gpui {
class Audio {
	struct Impl;
	std::unique_ptr<Impl> impl;
public:
	explicit Audio(agi::Context *context);
	~Audio();
	std::string Snapshot();
	std::string Render(json::Object const& request);
	std::string Interact(json::Object const& request);
};
}
