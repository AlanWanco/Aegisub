// Copyright (c) 2026, Aegisub Project
// SPDX-License-Identifier: BSD-3-Clause

#include "visual_render.h"

#include <algorithm>
#include <cmath>

namespace {
thread_local VisualRenderCapture *current_capture = nullptr;
constexpr std::array<double, 16> identity{1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
}

VisualRenderCapture::VisualRenderCapture() : previous(current_capture), transform(identity) { current_capture = this; }
VisualRenderCapture::~VisualRenderCapture() { current_capture = previous; }
VisualRenderCapture *VisualRenderCapture::Current() { return current_capture; }
void VisualRenderCapture::Reset() { transform = identity; }

void VisualRenderCapture::Style(float r, float g, float b, float a, float line_width, bool filled) {
	color = {r, g, b, a}; width = line_width; fill = filled;
}

void VisualRenderCapture::Multiply(std::array<double, 16> const& right) {
	std::array<double, 16> result{};
	for (int col = 0; col < 4; ++col)
		for (int row = 0; row < 4; ++row)
			for (int k = 0; k < 4; ++k)
				result[col * 4 + row] += transform[k * 4 + row] * right[col * 4 + k];
	transform = result;
}

json::Array VisualRenderCapture::Points(size_t dim, const float *points, size_t count) const {
	json::Array result;
	for (size_t i = 0; i < count; ++i) {
		double const x = points[i * dim], y = points[i * dim + 1], z = dim > 2 ? points[i * dim + 2] : 0;
		double w = transform[3] * x + transform[7] * y + transform[11] * z + transform[15];
		if (std::abs(w) < 0.0001) w = std::copysign(0.0001, w);
		double const projected_x = (transform[0] * x + transform[4] * y + transform[8] * z + transform[12]) / w;
		double const projected_y = (transform[1] * x + transform[5] * y + transform[9] * z + transform[13]) / w;
		json::Array point;
		point.emplace_back(std::clamp(projected_x, -1000000., 1000000.));
		point.emplace_back(std::clamp(projected_y, -1000000., 1000000.));
		result.emplace_back(std::move(point));
	}
	return result;
}

void VisualRenderCapture::Record(unsigned primitive, size_t dim, const float *points, size_t count) {
	if (!count || !points || dim < 2) return;
	json::Object draw;
	draw["kind"] = "path";
	draw["primitive"] = static_cast<int>(primitive);
	draw["fill"] = fill;
	draw["width"] = static_cast<double>(width);
	json::Array rgba;
	for (auto component : color) rgba.emplace_back(static_cast<double>(component));
	draw["color"] = std::move(rgba);
	draw["points"] = Points(dim, points, count);
	commands.emplace_back(std::move(draw));
}

void VisualRenderCapture::Mask(std::vector<float> const& points, std::vector<int> const& starts,
	std::vector<int> const& counts, float x, float y, float width, float height, bool outside) {
	json::Object draw;
	draw["kind"] = "mask";
	draw["outside"] = outside;
	json::Array rgba;
	for (auto component : color) rgba.emplace_back(static_cast<double>(component));
	draw["color"] = std::move(rgba);
	json::Array polygons;
	for (size_t i = 0; i < starts.size(); ++i)
		polygons.emplace_back(Points(2, points.data() + starts[i] * 2, counts[i]));
	draw["polygons"] = std::move(polygons);
	float rect[] = {x, y, x + width, y, x + width, y + height, x, y + height};
	draw["bounds"] = Points(2, rect, 4);
	commands.emplace_back(std::move(draw));
}

void VisualRenderCapture::Text(std::string const& text, int x, int y, int size, float r, float g, float b, float a) {
	json::Object draw;
	draw["kind"] = "text";
	draw["text"] = text;
	draw["x"] = x; draw["y"] = y; draw["size"] = size;
	json::Array rgba;
	for (float component : {r, g, b, a}) rgba.emplace_back(static_cast<double>(component));
	draw["color"] = std::move(rgba);
	commands.emplace_back(std::move(draw));
}
