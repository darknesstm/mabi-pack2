use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read};

pub const KEY_SALT: &str = "@6QeTuOaDgJlZcBm#9";

pub fn gen_header_key(input: &[u8]) -> Vec<u8> {
    (0..128)
        .map(|i| input[i % input.len()].wrapping_add(i as u8))
        .collect()
}

pub fn gen_header_offset(input: &[u8]) -> usize {
    let sum = input.iter().fold(0, |sum, c| sum + *c as usize);
    sum % 312 + 30
}

pub fn gen_entries_key(input: &[u8]) -> Vec<u8> {
    let len = input.len();
    (0..128)
        .map(|i| (i + (i % 3 + 2) * input[len - 1 - i % len] as usize) as u8)
        .collect()
}

pub fn gen_entries_offset(input: &[u8]) -> usize {
    let r = input.iter().fold(0, |r, c| r + *c as usize * 3);
    r % 212 + 42
}

pub fn gen_file_key(input: &[u8], key2: &[u8]) -> Vec<u8> {
    assert_eq!(key2.len(), 16);
    (0..128)
        .map(|i| {
            input[i % input.len()]
                .wrapping_mul(
                    key2[i % key2.len()]
                        .wrapping_sub(i as u8 / 5 * 5)
                        .wrapping_add(2)
                        .wrapping_add(i as u8),
                )
                .wrapping_add(i as u8)
        })
        .collect()
}

pub struct Decoder<'a, T: Read> {
    state1: [u32; 20],
    dec_stream: [u32; 16],
    cur_index: usize,
    rd: &'a mut T,

    left_buffer: [u8; 4],
    left_buffer_len: usize,
}

extern "C" {
    fn c_init_enc_state(state1: *mut u32, key: *const u8) -> i32;
    fn c_update_enc_state(state1: *mut u32, enc_stream: *mut u32) -> i32;
}

impl<'a, T: Read> Decoder<'a, T> {
    pub fn new(key: &[u8], reader: &'a mut T) -> Box<Self> {
        let mut r = Box::new(Decoder {
            state1: [0; 20],
            dec_stream: [0; 16],
            cur_index: 0,
            rd: reader,

            left_buffer: [0; 4],
            left_buffer_len: 0,
        });
        unsafe {
            c_init_enc_state(r.state1.as_mut_ptr(), key.as_ptr());
            r.update_enc_state();
        }
        r
    }

    fn update_enc_state(&mut self) {
        unsafe {
            c_update_enc_state(self.state1.as_mut_ptr(), self.dec_stream.as_mut_ptr());
        }
    }
}

impl<'a, T: Read> Read for Decoder<'a, T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let new_reading_len = buf.len() - self.left_buffer_len;
        let dec_block_len = (new_reading_len + 3) & 0usize.wrapping_sub(4);
        let mut ori_buff = vec![0u8; dec_block_len];
        self.rd.read_exact(&mut ori_buff)?;
        let mut reader_ori = Cursor::new(ori_buff);
        let mut writer_new = Cursor::new(Vec::<u8>::with_capacity(dec_block_len));
        for _ in 0..dec_block_len / 4 {
            let v = reader_ori
                .read_u32::<LittleEndian>()
                .unwrap()
                .wrapping_sub(self.dec_stream[self.cur_index]);
            writer_new.write_u32::<LittleEndian>(v)?;
            self.cur_index += 1;
            if self.cur_index >= 16 {
                self.update_enc_state();
                self.cur_index = 0;
            }
        }
        buf[..self.left_buffer_len].copy_from_slice(&self.left_buffer[..self.left_buffer_len]);
        let dec_block = writer_new.into_inner();
        buf[self.left_buffer_len..].copy_from_slice(&dec_block[..new_reading_len]);

        self.left_buffer_len = dec_block_len - new_reading_len;
        self.left_buffer[..self.left_buffer_len].copy_from_slice(&dec_block[new_reading_len..]);
        Ok(buf.len())
    }
}

/*fn show_slice_hex(v: &[u8]) {
    v.chunks(8).for_each(|s| {
        s.iter().for_each(|c| {
            print!("{:02X} ", c);
        });
        println!();
    });
}*/
