// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <memory>
#include <string>

namespace agi { struct Context; }

namespace gpui {
/// Whether a GPUI front end is connected, i.e. whether warnings should be sent
/// to it instead of opening wx dialogs the front end never shows.
bool FrontendAttached();

/// Queue a warning for the GPUI front end. The next document snapshot delivers
/// it exactly once; with no front end attached the notice is dropped.
void PushNotice(std::string title, std::string body);
/// Runs a command on the front end instead of opening the host's own dialog.
/// Commands the front end owns (the colour pickers today) use this so Lua and
/// the legacy menus do not pop a wx window over the GPUI frame.
void PushCommand(std::string name);

/// Publish a long-running task's progress so the front end can draw its own
/// progress card. `fraction` is 0..1, or negative for an indeterminate task.
/// The wx dialog is skipped entirely while a front end is attached.
void PushProgress(std::string title, std::string message, double fraction,
                  bool cancellable);
void ClearProgress();
/// Whether the user pressed cancel on the front end's progress card.
bool ProgressCancelled();
}

class GpuiBridge {
	struct Impl;
	std::unique_ptr<Impl> impl;

public:
	explicit GpuiBridge(agi::Context *context);
	~GpuiBridge();
	GpuiBridge(const GpuiBridge&) = delete;
	GpuiBridge& operator=(const GpuiBridge&) = delete;

	bool Start();
	void Stop();
	bool IsRunning() const;
	std::string Endpoint() const;
	std::string Token() const;
};
