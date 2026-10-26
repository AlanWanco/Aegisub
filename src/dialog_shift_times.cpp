// Copyright (c) 2011, Thomas Goyne <plorkyeran@aegisub.org>
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
//
// Aegisub Project http://www.aegisub.org/

#include "ass_dialogue.h"
#include "ass_file.h"
#include "compat.h"
#include "dialog_manager.h"
#include "format.h"
#include "include/aegisub/context.h"
#include "help_button.h"
#include "libresrc/libresrc.h"
#include "options.h"
#include "project.h"
#include "selection_controller.h"
#include "shift_times.h"
#include "subs_controller.h"
#include "timeedit_ctrl.h"

#include <libaegisub/ass/time.h>
#include <libaegisub/fs.h>
#include <libaegisub/io.h>
#include <libaegisub/log.h>
#include <libaegisub/path.h>
#include <libaegisub/signal.h>
#include <libaegisub/vfr.h>

#include <libaegisub/cajun/elements.h>
#include <libaegisub/cajun/reader.h>
#include <libaegisub/cajun/writer.h>

#include <wx/dialog.h>
#include <wx/listbox.h>
#include <wx/radiobox.h>
#include <wx/radiobut.h>
#include <wx/sizer.h>
#include <wx/statbox.h>
#include <wx/textctrl.h>

namespace {
class DialogShiftTimes final : public wxDialog {
	agi::Context *context;

	json::Array history;
	agi::vfr::Framerate fps;
	agi::signal::Connection timecodes_loaded_slot;
	agi::signal::Connection selected_set_changed_slot;

	TimeEdit *shift_time;
	wxTextCtrl *shift_frames;
	wxRadioButton *shift_by_time;
	wxRadioButton *shift_by_frames;
	wxRadioButton *shift_forward;
	wxRadioButton *shift_backward;
	wxRadioBox *selection_mode;
	wxRadioBox *time_fields;
	wxListBox *history_box;

	void LoadHistory();
	void Process(wxCommandEvent&);

	void OnClear(wxCommandEvent&);
	void OnByTime(wxCommandEvent&);
	void OnByFrames(wxCommandEvent&);
	void OnHistoryClick(wxCommandEvent&);

	void OnSelectedSetChanged();
	void OnTimecodesLoaded(agi::vfr::Framerate const& new_fps);

public:
	DialogShiftTimes(agi::Context *context);
	~DialogShiftTimes();
};

static wxString get_history_string(json::Object &obj) {
	auto filename = to_wx(obj["filename"]);
	if (filename.empty())
		filename = _("unsaved");

	auto shift_amount = to_wx(obj["amount"]);
	if (!obj["is by time"])
		shift_amount = fmt_plural(wxAtoi(shift_amount), "1 frame", "%s frames", shift_amount);

	wxString shift_direction = obj["is backward"] ? _("backward") : _("forward");

	int64_t time_field = obj["fields"];
	wxString fields =
		time_field == 0 ? _("s+e") :
		time_field == 1 ? _("s")   :
		                  _("e")   ;

	json::Array& sel = obj["selection"];
	wxString lines;

	int64_t sel_mode = obj["mode"];
	if (sel_mode == 0)
		lines = _("all");
	else if (sel_mode == 2) {
		if (!sel.empty())
			lines = fmt_tl("from %d onward", (int64_t)static_cast<json::Object&>(sel.front())["start"]);
	}
	else {
		lines += _("sel ");
		for (auto it = sel.begin(); it != sel.end(); ++it) {
			json::Object& range = *it;
			int64_t beg = range["start"];
			int64_t end = range["end"];
			if (beg == end)
				lines += std::to_wstring(beg);
			else
				lines += fmt_wx("%d-%d", beg, end);
			if (it + 1 != sel.end())
				lines += ";";
		}
	}

	return fmt_wx("%s, %s %s, %s, %s", filename, shift_amount, shift_direction, fields, lines);
}

DialogShiftTimes::DialogShiftTimes(agi::Context *context)
: wxDialog(context->parent, -1, _("Shift Times"))
, context(context)
, timecodes_loaded_slot(context->project->AddTimecodesListener(&DialogShiftTimes::OnTimecodesLoaded, this))
, selected_set_changed_slot(context->selectionController->AddSelectionListener(&DialogShiftTimes::OnSelectedSetChanged, this))
{
	SetIcons(GETICONS(shift_times_toolbutton));

	// Create static box sizers
	wxStaticBoxSizer *shift_by_sizer = new wxStaticBoxSizer(wxVERTICAL, this, _("Shift by"));
	wxStaticBoxSizer *history_sizer = new wxStaticBoxSizer(wxVERTICAL, this, _("Load from history"));
	wxWindow *shift_by_static_box = shift_by_sizer->GetStaticBox();
	wxWindow *history_static_box = history_sizer->GetStaticBox();

	// Create controls
	shift_by_time = new wxRadioButton(shift_by_static_box, -1, _("&Time: "), wxDefaultPosition, wxDefaultSize, wxRB_GROUP);
	shift_by_time->SetToolTip(_("Shift by time"));
	shift_by_time->Bind(wxEVT_RADIOBUTTON, &DialogShiftTimes::OnByTime, this);

	shift_by_frames = new wxRadioButton(shift_by_static_box, -1 , _("&Frames: "));
	shift_by_frames->SetToolTip(_("Shift by frames"));
	shift_by_frames->Bind(wxEVT_RADIOBUTTON, &DialogShiftTimes::OnByFrames, this);

	shift_time = new TimeEdit(shift_by_static_box, -1, context);
	shift_time->SetToolTip(_("Enter time in h:mm:ss.cs notation"));

	shift_frames = new wxTextCtrl(shift_by_static_box, -1);
	shift_frames->SetToolTip(_("Enter number of frames to shift by"));

	shift_forward = new wxRadioButton(shift_by_static_box, -1, _("For&ward"), wxDefaultPosition, wxDefaultSize, wxRB_GROUP);
	shift_forward->SetToolTip(_("Shifts subs forward, making them appear later. Use if they are appearing too soon."));

	shift_backward = new wxRadioButton(shift_by_static_box, -1, _("&Backward"));
	shift_backward->SetToolTip(_("Shifts subs backward, making them appear earlier. Use if they are appearing too late."));

	wxString selection_mode_vals[] = { _("&All rows"), _("Selected &rows"), _("Selection &onward") };
	selection_mode = new wxRadioBox(this, -1, _("Affect"), wxDefaultPosition, wxDefaultSize, 3, selection_mode_vals, 1);

	wxString time_field_vals[] = { _("Start a&nd End times"), _("&Start times only"), _("&End times only") };
	time_fields = new wxRadioBox(this, -1, _("Times"), wxDefaultPosition, wxDefaultSize, 3, time_field_vals, 1);

	history_box = new wxListBox(history_static_box, -1, wxDefaultPosition, wxSize(350, 100), 0, nullptr, wxLB_HSCROLL);

	wxButton *clear_button = new wxButton(history_static_box, -1, _("&Clear"));
	clear_button->Bind(wxEVT_BUTTON, &DialogShiftTimes::OnClear, this);

	// Set initial control states
	OnTimecodesLoaded(context->project->Timecodes());
	OnSelectedSetChanged();
	LoadHistory();

	shift_time->SetTime(OPT_GET("Tool/Shift Times/Time")->GetInt());
	*shift_frames << (int)OPT_GET("Tool/Shift Times/Frames")->GetInt();
	shift_by_frames->SetValue(!OPT_GET("Tool/Shift Times/ByTime")->GetBool() && shift_by_frames->IsEnabled());
	time_fields->SetSelection(OPT_GET("Tool/Shift Times/Type")->GetInt());
	selection_mode->SetSelection(OPT_GET("Tool/Shift Times/Affect")->GetInt());
	shift_backward->SetValue(OPT_GET("Tool/Shift Times/Direction")->GetBool());

	if (shift_by_frames->GetValue())
		shift_time->Disable();
	else
		shift_frames->Disable();

	// Position controls
	wxFlexGridSizer* shift_amount_sizer = new wxFlexGridSizer(2, 2, 5, 5);
	shift_amount_sizer->AddGrowableCol(1, 1);
	shift_amount_sizer->Add(shift_by_time, wxSizerFlags(0).Align(wxALIGN_CENTER_VERTICAL));
	shift_amount_sizer->Add(shift_time, wxSizerFlags().Expand());
	shift_amount_sizer->Add(shift_by_frames, wxSizerFlags(0).Align(wxALIGN_CENTER_VERTICAL));
	shift_amount_sizer->Add(shift_frames, wxSizerFlags().Expand());

	wxSizer *shift_direction_sizer = new wxBoxSizer(wxHORIZONTAL);
	shift_direction_sizer->Add(shift_forward, wxSizerFlags(1).Expand());
	shift_direction_sizer->Add(shift_backward, wxSizerFlags(1).Expand().Border(wxLEFT));

	shift_by_sizer->Add(shift_amount_sizer, wxSizerFlags().Expand());
	shift_by_sizer->Add(shift_direction_sizer, wxSizerFlags().Expand().Border(wxTOP));

	wxSizer *left_sizer = new wxBoxSizer(wxVERTICAL);
	left_sizer->Add(shift_by_sizer, wxSizerFlags().Expand().Border(wxBOTTOM));
	left_sizer->Add(selection_mode, wxSizerFlags().Expand().Border(wxBOTTOM));
	left_sizer->Add(time_fields, wxSizerFlags().Expand());

	history_sizer->Add(history_box, wxSizerFlags(1).Expand());
	history_sizer->Add(clear_button, wxSizerFlags().Expand().Border(wxTOP));

	wxSizer *top_sizer = new wxBoxSizer(wxHORIZONTAL);
	top_sizer->Add(left_sizer, wxSizerFlags().Border(wxALL & ~wxRIGHT).Expand());
	top_sizer->Add(history_sizer, wxSizerFlags().Border().Expand());

	wxSizer *main_sizer = new wxBoxSizer(wxVERTICAL);
	main_sizer->Add(top_sizer, wxSizerFlags().Border(wxALL & ~wxBOTTOM));
	main_sizer->Add(CreateButtonSizer(wxOK | wxCANCEL | wxHELP), wxSizerFlags().Right().Border());
	SetSizerAndFit(main_sizer);
	CenterOnParent();

	Bind(wxEVT_BUTTON, &DialogShiftTimes::Process, this, wxID_OK);
	Bind(wxEVT_BUTTON, std::bind(&HelpButton::OpenPage, "Shift Times"), wxID_HELP);
	shift_time->Bind(wxEVT_TEXT_ENTER, &DialogShiftTimes::Process, this);
	history_box->Bind(wxEVT_LISTBOX_DCLICK, &DialogShiftTimes::OnHistoryClick, this);
}

DialogShiftTimes::~DialogShiftTimes() {
	long shift;
	shift_frames->GetValue().ToLong(&shift);

	OPT_SET("Tool/Shift Times/Time")->SetInt(shift_time->GetTime());
	OPT_SET("Tool/Shift Times/Frames")->SetInt(shift);
	OPT_SET("Tool/Shift Times/ByTime")->SetBool(shift_by_time->GetValue());
	OPT_SET("Tool/Shift Times/Type")->SetInt(time_fields->GetSelection());
	OPT_SET("Tool/Shift Times/Affect")->SetInt(selection_mode->GetSelection());
	OPT_SET("Tool/Shift Times/Direction")->SetBool(shift_backward->GetValue());
}

void DialogShiftTimes::OnTimecodesLoaded(agi::vfr::Framerate const& new_fps) {
	fps = new_fps;
	if (fps.IsLoaded()) {
		shift_by_frames->Enable();
	}
	else {
		shift_by_time->SetValue(true);
		shift_by_frames->Disable();
		shift_time->Enable();
		shift_frames->Disable();
	}
}

void DialogShiftTimes::OnSelectedSetChanged() {
	if (context->selectionController->GetSelectedSet().empty()) {
		selection_mode->Enable(1, false);
		selection_mode->Enable(2, false);
		selection_mode->SetSelection(0);
	}
	else {
		selection_mode->Enable(1, true);
		selection_mode->Enable(2, true);
	}
}

void DialogShiftTimes::OnClear(wxCommandEvent &) {
	ClearShiftTimesHistory();
	history_box->Clear();
	history.clear();
}

void DialogShiftTimes::OnByTime(wxCommandEvent &) {
	shift_time->Enable(true);
	shift_frames->Enable(false);
}

void DialogShiftTimes::OnByFrames(wxCommandEvent &) {
	shift_time->Enable(false);
	shift_frames->Enable(true);
}

void DialogShiftTimes::OnHistoryClick(wxCommandEvent &evt) {
	size_t entry = evt.GetInt();
	if (entry >= history.size()) return;

	json::Object& obj = history[entry];
	if (obj["is by time"]) {
		shift_time->SetTime(agi::Time((std::string&)obj["amount"]));
		shift_by_time->SetValue(true);
		OnByTime(evt);
	}
	else {
		shift_frames->SetValue(to_wx((std::string&)obj["amount"]));
		if (shift_by_frames->IsEnabled()) {
			shift_by_frames->SetValue(true);
			OnByFrames(evt);
		}
	}

	if (obj["is backward"])
		shift_backward->SetValue(true);
	else
		shift_forward->SetValue(true);

	selection_mode->SetSelection((int64_t)obj["mode"]);
	time_fields->SetSelection((int64_t)obj["fields"]);
}

void DialogShiftTimes::LoadHistory() {
	history_box->Clear();
	history = LoadShiftTimesHistory();
	for (auto& entry : history)
		history_box->Append(get_history_string(entry));
}

void DialogShiftTimes::Process(wxCommandEvent &) {
	ShiftTimesSettings settings;
	settings.mode = selection_mode->GetSelection();
	settings.fields = time_fields->GetSelection();
	settings.backward = shift_backward->GetValue();
	settings.by_time = shift_by_time->GetValue();
	if (settings.by_time) settings.amount = shift_time->GetTime();
	else {
		long amount = 0;
		if (!shift_frames->GetValue().ToLong(&amount) || amount < 0 || amount > 35999990) {
			wxMessageBox(_("Enter a non-negative number of frames."), _("Shift Times"), wxOK | wxICON_ERROR, this);
			return;
		}
		settings.amount = static_cast<int>(amount);
	}
	ShiftTimes(context, settings);
	Close();
}
}

void ShowShiftTimesDialog(agi::Context *c) {
	c->dialog->Show<DialogShiftTimes>(c);
}
