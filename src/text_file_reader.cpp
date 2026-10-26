// Copyright (c) 2012, Thomas Goyne <plorkyeran@aegisub.org>
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

#include "text_file_reader.h"

#include <libaegisub/file_mapping.h>

#include <algorithm>
#include <string_view>

#include <boost/algorithm/string/trim.hpp>
#include <boost/algorithm/string/predicate.hpp>
#include <boost/interprocess/streams/bufferstream.hpp>

TextFileReader::TextFileReader(agi::fs::path const& filename, const char *encoding, bool trim)
: file(std::make_unique<agi::read_file_mapping>(filename))
, stream(std::make_unique<boost::interprocess::ibufferstream>(file->read(), file->size()))
, trim(trim)
{
	// Generic UTF-16/32 chooses its byte order from the BOM. The line
	// iterator must use that same order when constructing newline bytes.
	std::string_view bytes(file->read(), std::min<size_t>(file->size(), 4));
	if (boost::iequals(encoding, "utf-16") || boost::iequals(encoding, "Unicode (UTF-16)")) {
		if (bytes.starts_with("\xFF\xFE")) encoding = "utf-16le";
		else if (bytes.starts_with("\xFE\xFF")) encoding = "utf-16be";
	}
	else if (boost::iequals(encoding, "utf-32") || boost::iequals(encoding, "Unicode (UTF-32)")) {
		if (bytes.starts_with(std::string_view("\xFF\xFE\0\0", 4))) encoding = "utf-32le";
		else if (bytes.starts_with(std::string_view("\0\0\xFE\xFF", 4))) encoding = "utf-32be";
	}
	iter = agi::line_iterator<std::string>(*stream, encoding);
}

TextFileReader::~TextFileReader() {
}

std::string TextFileReader::ReadLineFromFile() {
	std::string str = *iter;
	++iter;
	if (trim)
		boost::trim(str);
	if (str.starts_with("\xEF\xBB\xBF"))
		str.erase(0, 3);
	return str;
}
