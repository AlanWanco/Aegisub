// Copyright (c) 2026, Aegisub contributors
// Distributed under the same license as ui_theme.h.

#pragma once

#include "ui_theme.h"

#include <wx/bmpbuttn.h>
#include <wx/button.h>
#include <wx/sizer.h>
#include <wx/statbmp.h>
#include <wx/statbox.h>
#include <wx/stattext.h>

namespace ui {
// Keep native dialog controls, default-button behaviour and platform button
// ordering. Only the content surfaces, spacing and artwork are shared.
inline wxStaticBoxSizer *DialogSection(wxWindow *parent, int orientation, wxString const& title) {
	auto section = new wxStaticBoxSizer(orientation, parent, title);
	ApplySurface(section->GetStaticBox());
	return section;
}

inline wxSizer *DialogHeading(wxWindow *parent, wxString const& title,
	std::string const& icon, wxBitmapBundle fallback) {
	auto row = new wxBoxSizer(wxHORIZONTAL);
	auto image = new wxStaticBitmap(parent, wxID_ANY, CommandIcon(icon, 24, fallback));
	image->Bind(ui::EVT_THEME_CHANGED, [image, icon, fallback](wxCommandEvent& event) {
		image->SetBitmap(CommandIcon(icon, 24, fallback));
		event.Skip();
	});
	auto label = new wxStaticText(parent, wxID_ANY, title);
	label->SetFont(parent->GetFont().Bold().Scaled(1.15));
	row->Add(image, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, parent->FromDIP(10));
	row->Add(label, 1, wxALIGN_CENTER_VERTICAL);
	return row;
}

inline void ButtonIcon(wxButton *button, std::string const& icon,
	wxBitmapBundle fallback, int size = 16) {
	button->SetBitmap(CommandIcon(icon, size, fallback));
	button->SetBitmapMargins(button->FromDIP(6), button->FromDIP(2));
	button->Bind(ui::EVT_THEME_CHANGED, [button, icon, fallback, size](wxCommandEvent& event) {
		button->SetBitmap(CommandIcon(icon, size, fallback));
		event.Skip();
	});
}

inline wxBitmapButton *IconButton(wxWindow *parent, std::string const& icon,
	wxBitmapBundle fallback, wxString const& label) {
	auto button = new wxBitmapButton(parent, wxID_ANY, CommandIcon(icon, 20, fallback),
		wxDefaultPosition, parent->FromDIP(wxSize(32, 32)));
	button->SetName(label);
	button->SetToolTip(label);
	ButtonIcon(button, icon, fallback, 20);
	return button;
}
}
