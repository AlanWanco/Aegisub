// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

/// Window interaction needed by the shared visual typesetting tools.
class VisualToolHost {
public:
	virtual ~VisualToolHost() = default;
	virtual void Render() = 0;
	virtual bool HasCapture() const = 0;
	virtual void CaptureMouse() = 0;
	virtual void ReleaseMouse() = 0;
	virtual void SetFocus() = 0;
	virtual void SetCrosshairCursor(bool hidden) = 0;
};
