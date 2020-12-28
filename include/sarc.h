#pragma once
#include "rust/cxx.h"
#include <oead/sarc.h>
#include <oead/yaz0.h>
#include <memory>

class Sarc
{
public:
    explicit Sarc(const rust::Slice<const uint8_t> data);
    uint32_t get_offset() const;
    size_t guess_align() const;
    uint16_t num_files() const;
    bool big_endian() const;
    bool files_eq(const Sarc& other) const;
    rust::Slice<const uint8_t> get_file_data(const rust::Str name) const;
    rust::Slice<const uint8_t> idx_file_data(const uint16_t idx) const;
    rust::Str idx_file_name(const uint16_t idx) const;

private:
    oead::Sarc inner;
};

std::unique_ptr<Sarc> sarc_from_binary(rust::Slice<const uint8_t> data);

