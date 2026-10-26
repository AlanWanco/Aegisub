// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <cstddef>
#include <vector>

namespace agi { struct Context; }
namespace cmd {
/// Line clipboard operations independent of the active wx text control.
void CopySelectedLines(agi::Context *context);
void CutSelectedLines(agi::Context *context);
void PasteLines(agi::Context *context);
std::size_t PasteLinesOver(agi::Context *context, std::vector<bool> fields);
}
