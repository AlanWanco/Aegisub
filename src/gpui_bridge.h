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
