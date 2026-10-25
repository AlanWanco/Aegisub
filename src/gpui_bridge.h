// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <memory>
#include <string>

namespace agi { struct Context; }

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
