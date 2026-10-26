// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <libaegisub/cajun/elements.h>
#include <memory>
#include <string>

namespace agi { struct Context; }
namespace gpui {
class Spelling {
	struct Impl;
	std::unique_ptr<Impl> impl;
public:
	explicit Spelling(agi::Context *context);
	~Spelling();
	std::string Request(json::Object const& request);
};
}
