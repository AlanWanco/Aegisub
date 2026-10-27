// Copyright (c) 2026, Aegisub contributors
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

#pragma once

#include "compat.h"
#include "options.h"
#include "ui_generated_icon_bitmaps.h"
#include "utils.h"

#include <string_view>
#include <wx/app.h>
#include <wx/bmpbndl.h>
#include <wx/button.h>
#include <wx/checkbox.h>
#include <wx/choice.h>
#include <wx/combobox.h>
#include <wx/control.h>
#include <wx/dialog.h>
#include <wx/frame.h>
#include <wx/listbox.h>
#include <wx/list.h>
#include <wx/menu.h>
#include <wx/menuitem.h>
#include <wx/mstream.h>
#include <wx/nonownedwnd.h>
#include <wx/panel.h>
#include <wx/settings.h>
#include <wx/sizer.h>
#include <wx/slider.h>
#include <wx/spinctrl.h>
#include <wx/statbox.h>
#include <wx/stc/stc.h>
#include <wx/textctrl.h>
#include <wx/toolbar.h>
#include <wx/toplevel.h>
#include <wx/window.h>
#ifdef __WXMSW__
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <winreg.h>
#include <dwmapi.h>
#ifndef DWMWA_USE_IMMERSIVE_DARK_MODE
#define DWMWA_USE_IMMERSIVE_DARK_MODE 20
#endif
#endif

// Keep the visual customizations here so upstream widgets and commands can
// continue to be used without a parallel application or document model.
namespace ui {
inline const wxEventTypeTag<wxCommandEvent> EVT_THEME_CHANGED(wxNewEventType());
enum class ThemeMode { System, Light, Dark };

inline ThemeMode Mode() {
	auto value = OPT_GET("App/Theme")->GetInt();
	if (value == static_cast<int>(ThemeMode::Light)) return ThemeMode::Light;
	if (value == static_cast<int>(ThemeMode::Dark)) return ThemeMode::Dark;
	return ThemeMode::System;
}

inline wxColour ColorFromHex(unsigned value) {
	return wxColour((value >> 16) & 255, (value >> 8) & 255, value & 255);
}

inline bool SystemIsDark() {
#ifdef __WXMSW__
	DWORD value = 1;
	DWORD size = sizeof(value);
	if (RegGetValueW(HKEY_CURRENT_USER,
		L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
		L"AppsUseLightTheme", RRF_RT_REG_DWORD, nullptr, &value, &size) == ERROR_SUCCESS)
		return value == 0;
#endif
#if wxCHECK_VERSION(3, 3, 0)
	return wxSystemSettings::GetAppearance().IsDark();
#else
	auto background = wxSystemSettings::GetColour(wxSYS_COLOUR_WINDOW);
	return background.Red() * 299 + background.Green() * 587 + background.Blue() * 114 < 128000;
#endif
}

inline bool IsDark() {
	auto mode = Mode();
	return mode == ThemeMode::Dark || (mode == ThemeMode::System && SystemIsDark());
}

inline wxColour Canvas() { return ColorFromHex(IsDark() ? 0x191F20 : 0xE7ECEA); }
inline wxColour Surface() { return ColorFromHex(IsDark() ? 0x22292A : 0xF5F8F7); }
inline wxColour Raised() { return ColorFromHex(IsDark() ? 0x2B3434 : 0xFFFFFF); }
inline wxColour Text() { return ColorFromHex(IsDark() ? 0xE5EFEC : 0x233631); }
inline wxColour Muted() { return ColorFromHex(IsDark() ? 0xA6B8B2 : 0x526761); }
inline wxColour Accent() { return ColorFromHex(IsDark() ? 0x78D8B9 : 0x08785D); }
inline wxColour OnAccent() { return ColorFromHex(IsDark() ? 0x142F27 : 0xFFFFFF); }
inline wxColour Selection() { return ColorFromHex(IsDark() ? 0x23483B : 0xD5EEE4); }
inline wxColour Border() { return ColorFromHex(IsDark() ? 0x3B4846 : 0xCCD7D2); }
inline wxColour Focus() { return ColorFromHex(IsDark() ? 0x82E6C5 : 0x006C50); }

inline wxColour ThemeColour(wxSystemColour index) {
	switch (index) {
		case wxSYS_COLOUR_WINDOW:
		case wxSYS_COLOUR_LISTBOX: return Raised();
		case wxSYS_COLOUR_DESKTOP:
		case wxSYS_COLOUR_APPWORKSPACE: return Canvas();
		case wxSYS_COLOUR_WINDOWTEXT:
		case wxSYS_COLOUR_LISTBOXTEXT: return Text();
		case wxSYS_COLOUR_HIGHLIGHT:
#if wxCHECK_VERSION(3, 3, 0)
		case wxSYS_COLOUR_LISTBOXHIGHLIGHT:
#endif
			return Accent();
		case wxSYS_COLOUR_HIGHLIGHTTEXT:
		case wxSYS_COLOUR_LISTBOXHIGHLIGHTTEXT: return OnAccent();
		case wxSYS_COLOUR_BTNFACE:
		case wxSYS_COLOUR_MENU:
		case wxSYS_COLOUR_MENUBAR:
		case wxSYS_COLOUR_ACTIVECAPTION:
		case wxSYS_COLOUR_GRADIENTACTIVECAPTION:
		case wxSYS_COLOUR_INACTIVECAPTION:
		case wxSYS_COLOUR_GRADIENTINACTIVECAPTION: return Surface();
		case wxSYS_COLOUR_BTNTEXT:
		case wxSYS_COLOUR_MENUTEXT:
		case wxSYS_COLOUR_CAPTIONTEXT:
		case wxSYS_COLOUR_INACTIVECAPTIONTEXT: return Text();
		case wxSYS_COLOUR_WINDOWFRAME:
		case wxSYS_COLOUR_ACTIVEBORDER:
		case wxSYS_COLOUR_INACTIVEBORDER:
		case wxSYS_COLOUR_BTNSHADOW:
		case wxSYS_COLOUR_3DLIGHT:
		case wxSYS_COLOUR_SCROLLBAR:
		case wxSYS_COLOUR_BTNHIGHLIGHT:
		case wxSYS_COLOUR_3DDKSHADOW:
		case wxSYS_COLOUR_MENUHILIGHT:
#if wxCHECK_VERSION(3, 3, 0)
		case wxSYS_COLOUR_GRIDLINES:
#endif
			return Border();
		case wxSYS_COLOUR_GRAYTEXT: return Muted();
		case wxSYS_COLOUR_HOTLIGHT: return Focus();
		case wxSYS_COLOUR_INFOTEXT: return Text();
		case wxSYS_COLOUR_INFOBK: return Surface();
		default: return wxSystemSettings::GetColour(index);
	}
}

inline wxColour OptionColour(std::string const& key);

inline bool IsCanvasWindow(wxWindow *window) {
	return window->GetName() == "AegisubMainCanvas" || window->GetName() == "AegisubAudioCanvas";
}

inline wxColour WindowBackground(wxWindow *window) {
	if (IsCanvasWindow(window)) return Canvas();
	if (wxDynamicCast(window, wxPanel)) return Surface();
	return Raised();
}

inline void ApplyAppearanceToMenu(wxMenu *menu);
inline void ApplyAppearanceToMenuBar(wxMenuBar *menu_bar);

inline void ApplyWindow(wxWindow *window) {
	if (!window || window->IsBeingDeleted()) return;
	if (auto frame = wxDynamicCast(window, wxFrame)) {
		ApplyAppearanceToMenuBar(frame->GetMenuBar());
		if (frame->GetStatusBar() && frame->GetStatusBar() != window)
			ApplyWindow(frame->GetStatusBar());
	}
#ifdef __WXMSW__
	if (wxDynamicCast(window, wxTopLevelWindow)) {
		BOOL const use_dark_title_bar = IsDark();
		DwmSetWindowAttribute(static_cast<HWND>(window->GetHandle()), DWMWA_USE_IMMERSIVE_DARK_MODE,
			&use_dark_title_bar, sizeof(use_dark_title_bar));
	}
#endif
	if (OPT_GET("App/Theme")->GetInt() == static_cast<int>(ThemeMode::System)) {
		window->SetBackgroundColour(wxNullColour);
		window->SetForegroundColour(wxNullColour);
	}
	else {
		if (wxDynamicCast(window, wxFrame) || wxDynamicCast(window, wxDialog))
			window->SetBackgroundColour(Canvas());
		else
			window->SetBackgroundColour(WindowBackground(window));
		window->SetForegroundColour(Text());
	}

	if (OPT_GET("App/Theme")->GetInt() != static_cast<int>(ThemeMode::System) &&
		(wxDynamicCast(window, wxComboBox) || wxDynamicCast(window, wxChoice) ||
		wxDynamicCast(window, wxTextCtrl) || wxDynamicCast(window, wxListBox) ||
		wxDynamicCast(window, wxSpinCtrl) || wxDynamicCast(window, wxSpinCtrlDouble))) {
		window->SetBackgroundColour(Raised());
		window->SetForegroundColour(Text());
	}
	else if (Mode() != ThemeMode::System && !wxDynamicCast(window, wxToolBar) &&
		!wxDynamicCast(window, wxButton) && !wxDynamicCast(window, wxCheckBox) &&
		!wxDynamicCast(window, wxSlider)) {
		window->SetForegroundColour(wxDynamicCast(window, wxStaticBox) ? Muted() : Text());
	}
	if (auto toolbar = wxDynamicCast(window, wxToolBar))
		toolbar->SetBackgroundColour(Mode() == ThemeMode::System ? wxNullColour : Surface());
	if (auto button = wxDynamicCast(window, wxButton)) {
		button->SetBackgroundColour(Mode() == ThemeMode::System ? wxNullColour : Surface());
		button->SetForegroundColour(Mode() == ThemeMode::System ? wxNullColour : Text());
	}
	if (auto checkbox = wxDynamicCast(window, wxCheckBox)) {
		checkbox->SetBackgroundColour(Mode() == ThemeMode::System ? wxNullColour : WindowBackground(window));
		checkbox->SetForegroundColour(Mode() == ThemeMode::System ? wxNullColour : Text());
	}
	if (auto slider = wxDynamicCast(window, wxSlider))
		slider->SetBackgroundColour(Mode() == ThemeMode::System ? wxNullColour : WindowBackground(window));
	if (wxDynamicCast(window, wxStaticBox) && Mode() != ThemeMode::System)
		window->SetForegroundColour(Muted());
	window->Refresh(false);
}

inline wxColour AudioCanvas() { return Canvas(); }
inline wxColour AudioWaveform() { return ColorFromHex(IsDark() ? 0x263332 : 0xF7FAF8); }
inline wxColour AudioAxis() { return ColorFromHex(IsDark() ? 0x647772 : 0x9BAAA4); }

inline wxColour AudioSchemeColour(std::string const& scheme, std::string const& group,
	std::string const& key) {
	auto color = OPT_GET("Colour/Schemes/" + scheme + "/" + group + "/" + key)->GetColor();
	bool recognized_default = false;
	if (group.starts_with("UI") && (scheme == "Green" || scheme == "Icy Blue")) {
		bool const focused = group == "UI Focused";
		agi::Color legacy, themed;
		if (key == "Light") {
			legacy = scheme == "Green"
				? (focused ? agi::Color(50, 255, 50) : agi::Color(0, 200, 0))
				: (focused ? agi::Color(205, 240, 226) : agi::Color(89, 145, 220));
			themed = focused ? agi::Color(130, 230, 197) : agi::Color(155, 170, 164);
			recognized_default = true;
		}
		else if (key == "Dark") {
			legacy = scheme == "Green" ? agi::Color(0, 10, 0) : agi::Color(8, 4, 13);
			themed = agi::Color(38, 51, 50);
			recognized_default = true;
		}
		else if (key == "Selection") {
			legacy = scheme == "Green" ? agi::Color(0, 80, 0)
				: (focused ? agi::Color(82, 107, 213) : agi::Color(65, 34, 103));
			themed = focused ? agi::Color(130, 230, 197) : agi::Color(35, 72, 59);
			recognized_default = true;
		}
		if (recognized_default && (color == legacy || color == themed)) {
			if (key == "Light") return focused ? Focus() : AudioAxis();
			if (key == "Dark") return AudioWaveform();
			return focused ? Focus() : Selection();
		}
	}
	return to_wx(color);
}

inline void ApplyThemeTree(wxWindow *window) {
	if (!window || window->IsBeingDeleted()) return;
	ApplyWindow(window);
	wxCommandEvent theme_event(EVT_THEME_CHANGED);
	theme_event.SetEventObject(window);
	window->GetEventHandler()->ProcessEventLocally(theme_event);
	for (auto child : window->GetChildren()) ApplyThemeTree(child);
}

inline void ApplyThemeToAllWindows() {
	for (auto window : wxTopLevelWindows) ApplyThemeTree(window);
}

inline void ApplyNativeAppearance() {
#if wxCHECK_VERSION(3, 3, 0) && defined(__WXMAC__)
	wxApp::Appearance appearance = wxApp::Appearance::System;
	if (Mode() == ThemeMode::Light) appearance = wxApp::Appearance::Light;
	if (Mode() == ThemeMode::Dark) appearance = wxApp::Appearance::Dark;
	wxTheApp->SetAppearance(appearance);
#endif
}

inline void SetThemeMode(ThemeMode mode) {
	OPT_SET("App/Theme")->SetInt(static_cast<int>(mode));
}

inline void ApplySurface(wxWindow *window, bool canvas = false) {
	if (!window) return;
	if (canvas) window->SetName("AegisubMainCanvas");
	ApplyWindow(window);
}

// Only replace the historical defaults. Explicit user color choices remain
// authoritative and continue to work through the existing preferences UI.
inline wxColour OptionColour(std::string const& key) {
	struct DefaultColour {
		std::string_view key;
		unsigned legacy, light, dark;
	};
	static constexpr DefaultColour defaults[] = {
		{"Colour/Subtitle Grid/Active Border", 0xFF5BEF, 0x126C58, 0x7BD7BC},
		{"Colour/Subtitle Grid/Background/Background", 0xFFFFFF, 0xFFFFFF, 0x242A2B},
		{"Colour/Subtitle Grid/Background/Comment", 0xD8DEF5, 0xF1F3F7, 0x30343D},
		{"Colour/Subtitle Grid/Background/Inframe", 0xFFFDEA, 0xF9F4DF, 0x39382B},
		{"Colour/Subtitle Grid/Background/Selected Comment", 0xD3EEEE, 0xE1EBED, 0x30454A},
		{"Colour/Subtitle Grid/Background/Selection", 0xCEFFE7, 0xDDEFE8, 0x29473E},
		{"Colour/Subtitle Grid/Collision", 0xFF0000, 0xAE3346, 0xFF9EA8},
		{"Colour/Subtitle Grid/Header", 0xA5CFE7, 0xEDF3F0, 0x303A37},
		{"Colour/Subtitle Grid/Left Column", 0xC4ECC9, 0xF5F8F6, 0x2A3130},
		{"Colour/Subtitle Grid/Lines", 0xBEBEBE, 0xE4EBE7, 0x394340},
		{"Colour/Subtitle Grid/Selection", 0x000000, 0x174B3E, 0xDCF5EA},
		{"Colour/Subtitle Grid/Standard", 0x000000, 0x253C38, 0xE4ECE9},
		{"Colour/Subtitle/Background", 0xFFFFFF, 0xFFFFFF, 0x242A2B},
		{"Colour/Subtitle/Syntax/Normal", 0x000000, 0x253C38, 0xE4ECE9},
		{"Colour/Subtitle/Syntax/Comment", 0x000000, 0x566B65, 0xAABAB5},
		{"Colour/Subtitle/Syntax/Drawing Command", 0x000000, 0x126C58, 0x7BD7BC},
		{"Colour/Subtitle/Syntax/Drawing X", 0x5A2828, 0x9B4551, 0xF2A3AD},
		{"Colour/Subtitle/Syntax/Drawing Y", 0x285A28, 0x326B42, 0xA0D3A8},
		{"Colour/Subtitle/Syntax/Brackets", 0x1432FF, 0x365FA6, 0x9ABCF5},
		{"Colour/Subtitle/Syntax/Error", 0xC80000, 0xAE3346, 0xFF9EA8},
		{"Colour/Subtitle/Syntax/Karaoke Template", 0x8000C0, 0x78539F, 0xCBB0EE},
		{"Colour/Subtitle/Syntax/Karaoke Variable", 0x8000C0, 0x78539F, 0xCBB0EE},
		{"Colour/Subtitle/Syntax/Line Break", 0xA0A0A0, 0x63766F, 0xACBBB4},
		{"Colour/Subtitle/Syntax/Parameters", 0x285A28, 0x326B42, 0xA0D3A8},
		{"Colour/Subtitle/Syntax/Slashes", 0xFF00C8, 0x9D467C, 0xE9ACD3},
		{"Colour/Subtitle/Syntax/Tags", 0x5A5A5A, 0x566B65, 0xAABAB5},
		{"Colour/Subtitle/Syntax/Background/Error", 0xFFC8C8, 0xFBE6E9, 0x59333A},
	};
	auto color = OPT_GET(key)->GetColor();
	unsigned value = (unsigned(color.r) << 16) | (unsigned(color.g) << 8) | color.b;
	for (auto const& entry : defaults) {
		if (entry.key != key || entry.legacy != value || color.a != 0) continue;
		bool const themed = Mode() != ThemeMode::System || IsDark();
		if (key == "Colour/Subtitle Grid/Background/Background" || key == "Colour/Subtitle/Background")
			return themed ? Raised() : ColorFromHex(entry.light);
		if (key == "Colour/Subtitle Grid/Background/Selection")
			return themed ? Selection() : ColorFromHex(entry.light);
		return ColorFromHex(IsDark() ? entry.dark : entry.light);
	}
	if (Mode() == ThemeMode::System) return to_wx(color);
	return wxColour(color.r, color.g, color.b, 255 - color.a);
}

inline void ApplyAppearanceToMenu(wxMenu *menu) {
#ifdef __WXMSW__
	if (!menu) return;
	for (auto item : menu->GetMenuItems())
		if (item->GetSubMenu()) ApplyAppearanceToMenu(item->GetSubMenu());
#else
	(void)menu;
#endif
}

inline void ApplyAppearanceToMenuBar(wxMenuBar *menu_bar) {
#ifdef __WXMSW__
	if (!menu_bar) return;
	if (Mode() == ThemeMode::System) {
		menu_bar->SetBackgroundColour(wxNullColour);
		menu_bar->SetForegroundColour(wxNullColour);
		return;
	}
	menu_bar->SetBackgroundColour(Surface());
	menu_bar->SetForegroundColour(Text());
	for (int i = 0; i < static_cast<int>(menu_bar->GetMenuCount()); ++i)
		ApplyAppearanceToMenu(menu_bar->GetMenu(i));
#else
	(void)menu_bar;
#endif
}

inline wxBitmapBundle CommandIcon(std::string_view command, int size, wxBitmapBundle fallback, bool active = false) {
#ifndef wxHAS_SVG
	bool const dark = IsDark();
	for (auto const& bitmap : generated_icon_bitmaps) {
		if (bitmap.command != command || bitmap.active != active || bitmap.dark != dark || bitmap.logical_size > size) continue;
		wxVector<wxBitmap> images;
		for (size_t i = 0; i < bitmap.count; ++i) {
			wxMemoryInputStream stream(bitmap.images[i].data, bitmap.images[i].size);
			wxImage image(stream, wxBITMAP_TYPE_PNG);
			if (!image.IsOk()) continue;
			auto result = wxBitmap(image);
			result.SetScaleFactor(double(bitmap.images[i].scale) / bitmap.logical_size);
			images.push_back(result);
		}
		if (!images.empty()) return wxBitmapBundle::FromBitmaps(images);
	}
#endif
#ifdef wxHAS_SVG
	struct Icon { std::string_view command, paths; };
	static constexpr Icon icons[] = {
		{"ui/new", "<path d='M12 4v16M4 12h16'/>"},
		{"ui/edit", "<path d='m4 16 12-12 4 4L8 20H4zM14 6l4 4'/>"},
		{"ui/copy", "<rect x='8' y='8' width='12' height='13' rx='2'/><path d='M16 8V3H3v13h5'/>"},
		{"ui/delete", "<path d='M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7m4-7v7'/>"},
		{"ui/up", "<path d='M12 20V4m-7 7 7-7 7 7'/>"},
		{"ui/down", "<path d='M12 4v16m-7-7 7 7 7-7'/>"},
		{"ui/top", "<path d='M4 3h16M12 21V7m-6 6 6-6 6 6'/>"},
		{"ui/bottom", "<path d='M4 21h16M12 3v14m-6-6 6 6 6-6'/>"},
		{"ui/sort", "<path d='M5 4v16m-3-3 3 3 3-3M11 5h4m-4 7h7m-7 7h10'/>"},
		{"ui/left", "<path d='M20 12H4m7-7-7 7 7 7'/>"},
		{"ui/right", "<path d='M4 12h16m-7-7 7 7-7 7'/>"},
		{"ui/import", "<path d='M5 9V3h10l4 4v14H5v-5M15 3v5h4M2 12h10m-3-3 3 3-3 3'/>"},
		{"ui/preview", "<path d='M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z'/><circle cx='12' cy='12' r='3'/>"},
		{"ui/accept", "<path d='m4 12 5 5L20 6'/>"},
		{"ui/insert", "<rect x='3' y='3' width='18' height='18' rx='2'/><path d='M8 8h8M8 12h4m4 1v6m-3-3h6'/>"},
		{"ui/audio", "<path d='M3 9h4l5-5v16l-5-5H3zM16 8a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14'/>"},
		{"subtitle/new", "<path d='M9 3H5v18h14V7l-4-4H9z'/><path d='M14 3v5h5M8 13h8M8 17h5'/>"},
		{"subtitle/open", "<path d='M3 8V5h6l2 3h10l-3 12H3V8z'/><path d='M3 10h17'/>"},
		{"subtitle/save", "<path d='M4 3h13l3 3v15H4zM8 3v6h8V3M8 21v-8h8v8'/>"},
		{"video/jump", "<rect x='3' y='5' width='18' height='14' rx='2'/><path d='m10 9 5 3-5 3z'/>"},
		{"video/zoom/in", "<circle cx='10' cy='10' r='6'/><path d='m15 15 6 6M7 10h6M10 7v6'/>"},
		{"video/zoom/out", "<circle cx='10' cy='10' r='6'/><path d='m15 15 6 6M7 10h6'/>"},
		{"video/jump/start", "<path d='M5 5v14m14-14L8 12l11 7z'/>"},
		{"video/jump/end", "<path d='M19 5v14M5 5l11 7-11 7z'/>"},
		{"time/snap/start_video", "<path d='M5 4v16M20 8h-9l4-4m-4 4 4 4M9 17h11'/>"},
		{"time/snap/end_video", "<path d='M19 4v16M4 8h9L9 4m4 4-4 4M4 17h11'/>"},
		{"subtitle/select/visible", "<path d='M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z'/><circle cx='12' cy='12' r='3'/>"},
		{"time/snap/scene", "<rect x='3' y='5' width='18' height='14' rx='2'/><path d='M8 5v14M16 5v14M3 10h5m8 4h5'/>"},
		{"time/frame/current", "<path d='M4 4v5M4 4h5m11 0h-5m5 0v5M4 20v-5m0 5h5m11 0h-5m5 0v-5'/><path d='M8 12h8m-4-4v8'/>"},
		{"tool/style/manager", "<path d='m5 20 7-16 7 16M8 14h8M4 21h16'/>"},
		{"subtitle/properties", "<rect x='4' y='3' width='16' height='18' rx='2'/><path d='M8 8h8M8 12h8M8 16h5'/>"},
		{"subtitle/attachment", "<path d='m8 12 6-6a3 3 0 0 1 4 4L9 19a5 5 0 0 1-7-7l10-10m-6 14 10-10'/>"},
		{"tool/font_collector", "<path d='M4 6h16M12 6v14M8 20h8M4 3v6M20 3v6'/>"},
		{"am/meta", "<path d='m8 4-5 8 5 8m8-16 5 8-5 8m-3-17-2 18'/>"},
		{"time/shift", "<circle cx='10' cy='10' r='7'/><path d='M10 6v4l3 2m2 7h7m-3-3 3 3-3 3'/>"},
		{"tool/style/assistant", "<path d='M4 20 15 9m-3-3 6 6M5 3v4M3 5h4m12 10v6m-3-3h6M17 3v4m-2-2h4'/>"},
		{"tool/translation_assistant", "<path d='M3 5h12M9 3v2M5 5c1 6 4 8 8 10M12 5c-1 5-4 8-9 10m11 6 4-11 4 11m-6-4h4'/>"},
		{"tool/resampleres", "<rect x='3' y='3' width='12' height='12' rx='1'/><path d='M9 21h12V9M9 9l12 12m-5 0h5v-5'/>"},
		{"tool/time/postprocess", "<path d='M4 6h16M4 12h16M4 18h16'/><circle cx='8' cy='6' r='2'/><circle cx='16' cy='12' r='2'/><circle cx='10' cy='18' r='2'/>"},
		{"tool/time/kanji", "<rect x='3' y='3' width='18' height='18' rx='2'/><path d='M7 8h10M12 5v13M8 11l8 6m0-6-8 6'/>"},
		{"subtitle/spellcheck", "<path d='m3 13 4-9 4 9M5 10h4m4 6 3 3 6-7'/>"},
		{"app/options", "<path d='M4 6h16M4 12h16M4 18h16M8 3v6m8 0v6M10 15v6'/>"},
		{"grid/tag/cycle_hiding", "<path d='m8 6-5 6 5 6m8-12 5 6-5 6M10 20l4-16'/>"},
		{"video/play", "<path d='m7 4 14 8-14 8z'/>"},
		{"video/play/line", "<path d='m6 5 11 7-11 7zM21 5v14'/>"},
		{"video/stop", "<rect x='5' y='5' width='14' height='14' rx='1'/>"},
		{"video/opt/autoscroll", "<path d='M4 7h16M4 12h10M4 17h10m3-3 4 3-4 3'/>"},
		{"audio/play/selection", "<path d='m7 4 14 8-14 8z'/>"},
		{"audio/play/line", "<path d='m6 5 11 7-11 7zM21 5v14'/>"},
		{"audio/stop", "<rect x='5' y='5' width='14' height='14' rx='1'/>"},
		{"time/prev", "<path d='M5 5v14m14-14L8 12l11 7z'/>"},
		{"time/next", "<path d='M19 5v14M5 5l11 7-11 7z'/>"},
		{"audio/commit", "<path d='m4 12 5 5L20 6'/>"},
		{"audio/go_to", "<path d='M12 3v18M3 12h18'/><circle cx='12' cy='12' r='6'/>"},
		{"audio/play/selection/before", "<path d='M14 4v16M3 12h8m-4-4 4 4-4 4M19 7v10'/>"},
		{"audio/play/selection/after", "<path d='M10 4v16m3-8h8m-4-4 4 4-4 4M5 7v10'/>"},
		{"audio/play/selection/begin", "<path d='M5 4v16m4-14 10 6-10 6z'/>"},
		{"audio/play/selection/end", "<path d='M19 4v16M5 6l10 6-10 6z'/>"},
		{"audio/play/to_end", "<path d='m3 6 9 6-9 6zm9 0 9 6-9 6z'/>"},
		{"time/lead/in", "<path d='M19 4v16M3 12h12m-5-5 5 5-5 5'/>"},
		{"time/lead/out", "<path d='M5 4v16m4-8h12m-5-5 5 5-5 5'/>"},
		{"audio/opt/autocommit", "<circle cx='12' cy='12' r='9'/><path d='m7 12 3 3 7-7'/>"},
		{"audio/opt/autonext", "<path d='M3 6h8M3 12h6M3 18h8m3-12 6 6-6 6M9 12h11'/>"},
		{"audio/opt/autoscroll", "<path d='M4 7h16M4 12h10M4 17h10m3-3 4 3-4 3'/>"},
		{"audio/opt/spectrum", "<path d='M4 14v4m4-9v9m4-14v14m4-11v11m4-5v5'/>"},
		{"app/toggle/global_hotkeys", "<rect x='3' y='6' width='18' height='13' rx='2'/><path d='M7 10h1m3 0h1m3 0h1M7 14h1m3 0h5'/>"},
		{"audio/karaoke", "<rect x='9' y='3' width='6' height='12' rx='3'/><path d='M5 11a7 7 0 0 0 14 0M12 18v3m-4 0h8'/>"},
		{"audio/opt/vertical_link", "<path d='m10 8 3-3a4 4 0 0 1 6 6l-3 3m-2 2-3 3a4 4 0 0 1-6-6l3-3m1 5 6-6'/>"},
		{"video/tool/cross", "<path d='M12 2v20M2 12h20'/><circle cx='12' cy='12' r='5'/>"},
		{"video/tool/drag", "<path d='M12 3v18M3 12h18M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3m12-6 3 3-3 3'/>"},
		{"video/tool/rotate/z", "<path d='M4 10a8 8 0 1 1 2 8M4 3v7h7'/><path d='M11 8h5l-5 8h5'/>"},
		{"video/tool/rotate/xy", "<ellipse cx='12' cy='12' rx='9' ry='4'/><ellipse cx='12' cy='12' rx='4' ry='9'/>"},
		{"video/tool/scale", "<rect x='3' y='9' width='12' height='12' rx='1'/><path d='M10 3h11v11m0-11L9 15'/>"},
		{"video/tool/clip", "<path d='M7 3v14h14M3 7h14v14'/>"},
		{"video/tool/vector_clip", "<path d='m5 5 14 3-4 12-12-6z'/><rect x='3' y='3' width='4' height='4'/><rect x='17' y='6' width='4' height='4'/><rect x='13' y='18' width='4' height='4'/>"},
		{"video/tool/vclip/drag", "<path d='m5 5 14 3-4 12-12-6z'/><path d='m4 4 5 2M4 4v5m16 0V7h-3m-1 14h4v-4'/>"},
		{"video/tool/vclip/line", "<rect x='3' y='3' width='4' height='4'/><rect x='17' y='17' width='4' height='4'/><path d='m7 7 10 10'/>"},
		{"video/tool/vclip/bicubic", "<circle cx='5' cy='18' r='2'/><circle cx='19' cy='6' r='2'/><path d='M7 18c7 0 5-12 10-12'/>"},
		{"video/tool/vclip/convert", "<path d='M5 6h14M5 18h14M7 8v8m10-8v8'/>"},
		{"video/tool/vclip/insert", "<path d='M5 5h14v14H5zM12 9v6m-3-3h6'/>"},
		{"video/tool/vclip/remove", "<path d='M5 5 19 19M19 5 5 19'/>"},
		{"video/tool/vclip/freehand", "<path d='M3 18c4-13 5 5 9-5s6 7 9-7'/><path d='M3 21h18'/>"},
		{"video/tool/vclip/freehand_smooth", "<path d='M3 17c5 2 5-11 10-8s4 7 8-3'/><circle cx='3' cy='17' r='1'/><circle cx='21' cy='6' r='1'/>"},
		{"help/video", "<circle cx='12' cy='12' r='9'/><path d='M9 9a3 3 0 1 1 5 2c-2 1-2 2-2 3m0 3h.01'/>"},
		{"edit/color/primary", "<path d='m5 17 7-14 7 14M8 12h8'/><path d='M4 21h16' stroke='#76B9A3' stroke-width='3'/>"},
		{"edit/color/secondary", "<path d='m5 17 7-14 7 14M8 12h8'/><path d='M4 21h16' stroke='#C9A56C' stroke-width='3'/>"},
		{"edit/color/outline", "<path d='m5 17 7-14 7 14M8 12h8'/><rect x='3' y='19' width='18' height='3'/>"},
		{"edit/color/shadow", "<path d='m5 17 7-14 7 14M8 12h8'/><path d='M4 20h13m-10 2h13' opacity='.6'/>"},
		{"edit/style/bold", "<path d='M7 4h6a4 4 0 0 1 0 8H7zm0 8h7a4 4 0 0 1 0 8H7z'/>"},
		{"edit/style/italic", "<path d='M10 4h10M4 20h10M15 4 9 20'/>"},
		{"edit/style/underline", "<path d='M6 3v8a6 6 0 0 0 12 0V3M4 21h16'/>"},
		{"edit/style/strikeout", "<path d='M17 6c-1-4-10-4-10 1 0 2 2 3 5 4m5 4c2 6-9 7-11 2M3 12h18'/>"},
		{"edit/font", "<path d='M4 6h16M12 6v14M8 20h8M4 3v6M20 3v6'/>"},
		{"grid/line/next/create", "<rect x='3' y='3' width='18' height='18' rx='2'/><path d='M12 7v10M7 12h10'/>"},
	};
	for (auto const& icon : icons) {
		if (icon.command != command) continue;
		auto foreground = active ? ColorFromHex(IsDark() ? 0x173D32 : 0xFFFFFF) : Text();
		auto color = foreground.GetAsString(wxC2S_HTML_SYNTAX).ToStdString();
		std::string svg = "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24' fill='none' stroke='" + color + "' stroke-width='1.7' stroke-linecap='round' stroke-linejoin='round'>";
		if (active) {
			auto accent = Accent().GetAsString(wxC2S_HTML_SYNTAX).ToStdString();
			svg += "<rect x='0' y='0' width='24' height='24' rx='4' fill='" + accent + "' stroke='none'/>";
			svg += "<g transform='translate(3 3) scale(.75)'>";
		}
		svg += icon.paths;
		if (active) svg += "</g>";
		svg += "</svg>";
		return wxBitmapBundle::FromSVG(svg.c_str(), wxSize(size, size));
	}
#endif
	return fallback;
}
}
