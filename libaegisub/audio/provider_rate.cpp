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

#include "libaegisub/audio/provider_rate.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <vector>

namespace agi {
PlaybackAudioProvider::PlaybackAudioProvider(AudioProvider const *source, double rate)
: source(source), rate(rate)
{
	if (!source || !std::isfinite(rate) || rate < 0.5 || rate > 2.0)
		throw InternalError("Playback rate must be between 0.5 and 2.0");
	if (source->GetBytesPerSample() != 2 || source->AreSamplesFloat() || source->GetChannels() != 1)
		throw InternalError("Playback rate requires converted 16-bit mono audio");

	channels = 1;
	bytes_per_sample = 2;
	sample_rate = source->GetSampleRate();
	num_samples = SourceToPlayback(source->GetNumSamples());
	decoded_samples = SourceToPlayback(source->GetDecodedSamples());
}

int64_t PlaybackAudioProvider::SourceToPlayback(int64_t sample) const {
	return static_cast<int64_t>(std::ceil(sample / rate));
}

int64_t PlaybackAudioProvider::PlaybackToSource(int64_t sample) const {
	return static_cast<int64_t>(std::llround(sample * rate));
}

void PlaybackAudioProvider::FillBuffer(void *buf, int64_t start, int64_t count) const {
	if (rate == 1.0) {
		source->GetAudio(buf, start, count);
		return;
	}

	// Use absolute positions so separate backend requests produce exactly the
	// same samples as one contiguous read. Bound temporary storage for large
	// audio callbacks, and retain the interpolation neighbour at each edge.
	auto output = static_cast<int16_t *>(buf);
	constexpr int64_t block_size = 4096;
	std::vector<int16_t> input;
	while (count > 0) {
		int64_t size = std::min(count, block_size);
		int64_t first = static_cast<int64_t>(std::floor(start * rate));
		int64_t last = static_cast<int64_t>(std::floor((start + size - 1) * rate)) + 1;
		input.resize(last - first + 1);
		source->GetAudio(input.data(), first, input.size());
		for (int64_t i = 0; i < size; ++i) {
			double position = (start + i) * rate;
			int64_t index = static_cast<int64_t>(std::floor(position)) - first;
			double fraction = position - std::floor(position);
			double sample = input[index] + fraction * (input[index + 1] - input[index]);
			output[i] = static_cast<int16_t>(std::lround(sample));
		}
		start += size;
		output += size;
		count -= size;
	}
}
}
