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

#include <libaegisub/audio/provider.h>

namespace agi {
/// A playback-only view of converted audio. The project keeps the original
/// provider for waveform rendering, exports, and all subtitle timecodes.
/// Construct a new view while the player is stopped to change the rate.
class PlaybackAudioProvider final : public AudioProvider {
	AudioProvider const *source;
	double rate;

	void FillBuffer(void *buf, int64_t start, int64_t count) const override;

public:
	PlaybackAudioProvider(AudioProvider const *source, double rate);

	int64_t SourceToPlayback(int64_t sample) const;
	int64_t PlaybackToSource(int64_t sample) const;
	double GetRate() const { return rate; }
};
}
