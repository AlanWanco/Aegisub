// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#pragma once

#include <libaegisub/cajun/elements.h>
#include <array>
#include <string>
#include <vector>

/// Records the existing visual tools' drawing for a frontend without OpenGL.
/// The default wx rendering path remains immediate OpenGL.
class VisualRenderCapture {
	VisualRenderCapture *previous;
	std::array<double, 16> transform;
	std::array<float, 4> color{1, 1, 1, 1};
	float width = 1;
	bool fill = false;
	json::Array commands;
	json::Array Points(size_t dim, const float *points, size_t count) const;
public:
	VisualRenderCapture();
	~VisualRenderCapture();
	static VisualRenderCapture *Current();
	void Style(float r, float g, float b, float a, float width, bool fill);
	void Multiply(std::array<double, 16> const& matrix);
	void Reset();
	void Record(unsigned primitive, size_t dim, const float *points, size_t count);
	void Mask(std::vector<float> const& points, std::vector<int> const& starts,
		std::vector<int> const& counts, float x, float y, float width, float height, bool outside);
	void Text(std::string const& text, int x, int y, int size, float r, float g, float b, float a);
	json::Array TakeCommands() { return std::move(commands); }
};
