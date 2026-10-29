// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <cstddef>
#include <optional>
#include <string>
#include <vector>

#include <libaegisub/color.h>

namespace agi { struct Context; }
namespace cmd {
/// Line clipboard operations independent of the active wx text control.
void CopySelectedLines(agi::Context *context);
void CutSelectedLines(agi::Context *context);
void PasteLines(agi::Context *context);
std::size_t PasteLinesOver(agi::Context *context, std::vector<bool> fields);

/// Starts a colour pick for one ASS override slot ("primary", "secondary",
/// "outline" or "shadow") and returns the colour the dialog should open on.
/// The pick stays pending until it is applied or cancelled.
std::optional<agi::Color> BeginColorPick(agi::Context *context, std::string const& kind);
void ApplyColorPick(agi::Context *context, agi::Color color);
void CancelColorPick(agi::Context *context);

/// The font a line renders with at the cursor: style defaults overlaid with the
/// \fn, \fs, \b, \i and \u tags that apply there.
struct LineFont {
	std::string face;
	int size = 0;
	bool bold = false;
	bool italic = false;
	bool underline = false;
	bool operator==(LineFont const&) const = default;
};
std::optional<LineFont> CurrentFont(agi::Context *context);
/// Applies font tags to the selection, writing only the fields that differ.
void ApplyFont(agi::Context *context, LineFont const& font);
}
