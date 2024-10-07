// Return type is (n bytes, pack type, size of item)
pub fn pack_item(pack_data: &mut <Vec<u8> as IntoIterator>::IntoIter) -> (u64, u8, u128) {
    let mut n = 1;
    let mut size = 0;

    // Init: len MSB + 3bit Type + 4bit Size
    let init_byte = pack_data.next().unwrap();
    let init_type = (init_byte >> 4) & 7;
    let init_size = init_byte & 15;
    size += init_size as u128;
    let mut msb = (init_byte & 128) >> 7 == 1;
    while msb {
        let byte = pack_data.by_ref().next().unwrap() as u128;
        let v_size = (byte & 127) << (4 + (n - 1) * 7);
        size += v_size;
        msb = (byte & 128) >> 7 == 1;
        n += 1;
    }
    return (n, init_type, size);
}

pub fn delta_offset(pack_data: &mut <Vec<u8> as IntoIterator>::IntoIter) -> (u64, u128) {
    let mut c = pack_data.next().unwrap() as u128;
    let mut n = 1;
    let mut ofs = c & 127;
    while c & 128 != 0 {
        ofs += 1;
        n += 1;
        c = pack_data.by_ref().next().unwrap() as u128;
        ofs = (ofs << 7) + (c & 127);
    };

    (n, ofs)
}
// Return is number of bytes read and value of size
pub fn delta_buf_length(buf : &mut <Vec<u8> as IntoIterator>::IntoIter) -> (u128, u128) {
    
    let mut n = 0;
    let mut msb = true;
    let mut size: u128 = 0;
    while msb  {
        let byte = buf.next().unwrap();
        msb = byte & 128 == 128;
        size = ((byte as u128 & 127) << (7*n) ) | size;
        n += 1;
    };
    (n, size)
}

pub fn delta_copy_length(buf : &mut <Vec<u8> as IntoIterator>::IntoIter, byte: u8) -> (u128, u128) {
    // Offset
    let mut ofs: u128 = 0;
    let mut size: u128 = 0;
    if byte & 0x01 == 0x01 {
        let ofs1 = buf.next().unwrap() as u128;
        ofs = ofs1;
    }
    if byte & 0x02 == 0x02{
        let ofs2 = buf.next().unwrap() as u128;
        ofs = (ofs2 << 8) | ofs;
    }
    if byte & 0x04 == 0x04{
        let ofs3 = buf.next().unwrap() as u128;
        ofs = (ofs3 << 16) | ofs;
    }
    if byte & 0x08 == 0x08{
        let ofs4 = buf.next().unwrap() as u128;
        ofs = (ofs4 << 24) | ofs;
    }


    if byte & 0x10 == 0x10{
        let s1 = buf.next().unwrap() as u128;
        size = s1;
    }
    if byte & 0x20 == 0x20{
        let s2 = buf.next().unwrap() as u128;
        size = (s2 << 8) | size;
    }
    if byte & 0x40 == 0x40{
        let s3 = buf.next().unwrap() as u128;
        size = (s3 << 16) | size;
    }

    

    (ofs, size)
}
