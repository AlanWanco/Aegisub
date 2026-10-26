// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <cstdint>
#include <string>
#include <vector>

struct AutosaveVersion {
	std::string filename;
	std::string display;
	std::string kind;
	int64_t timestamp;
};

struct AutosaveFile {
	std::string name;
	std::vector<AutosaveVersion> versions;
};

// The wx recovery dialog and GPUI use the same configured folders, filename
// parsing, grouping, and newest-first ordering.
std::vector<AutosaveFile> GetAutosaveFiles();
