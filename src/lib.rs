use std::error::Error;

use nom::{
    bytes::complete::{tag, take_while},
    character::complete::{char, digit1, line_ending, space1, one_of, anychar, not_line_ending},
    combinator::{map_res, recognize, opt},
    multi::{many1, many0, many_m_n},
    sequence::{preceded, terminated},
    IResult, branch::alt, Parser,
};

const HEAP_V2_HEADER: &str = "heap_v2";
const MAPPED_LIBRARIES_HEADER: &str = "MAPPED_LIBRARIES:\n";

/// A Jemalloc HeapV2 Profile
#[derive(Debug)]
pub struct Profile<'a> {
    pub sampling_rate: u64,
    pub totals: Vec<Thread<'a>>,
    pub stacks: Vec<Stack<'a>>,
    pub mapped_libraries: Vec<MappedLibrary<'a>>
}

impl<'a> Profile<'a> {
    pub fn parse(profile: &'a str) -> Result<Self, Box<dyn Error>> {
        if !profile.starts_with(HEAP_V2_HEADER) {
            return Err("Only HEAP V2 profiles are supported".into())
        }

        let (_, profile) = parse_profile(profile).map_err(|_| "failed to parse heap_v2 profile")?;

        Ok(profile)
    }
}

#[derive(Debug)]
pub struct Stack<'a> {
    pub addrs: Vec<i64>,
    pub threads: Vec<Thread<'a>>,
}

#[derive(Debug)]
pub struct Thread<'a> {
    pub id: &'a str,
    pub inuse_count: u64,
    pub insuse_space: u64,
    pub alloc_count: u64,
    pub alloc_space: u64,
}

#[derive(Debug)]
pub struct MappedLibrary<'a> {
    first: i64,
    last: i64,
    path: &'a str,
}

fn parse_profile(input: &str) -> IResult<&str, Profile> {
    let (input, sampling_rate) = parse_header(input)?;
    let (input, _) = line_ending(input)?;
    let (input, threads) = many1(terminated(preceded(space1, parse_thread), line_ending))(input)?;
    let (input, stacks) = many1(parse_stack)(input)?;

    let (input, _) = many0(line_ending)(input)?;

    let (input, _) = tag(MAPPED_LIBRARIES_HEADER)(input)?;

    let (input, mapped_libraries) = many0(terminated(parse_mapped_library, line_ending))(input)?;
    let mapped_libraries = mapped_libraries.into_iter().filter(|lib| !lib.path.is_empty()).collect();

    let profile = Profile {
        sampling_rate,
        totals: threads,
        stacks,
        mapped_libraries
    };

    Ok((input, profile))
}

fn parse_header(input: &str) -> IResult<&str, u64> {
    let (input, _) = tag("heap_v2/")(input)?;
    map_res(digit1, |digit_str: &str| digit_str.parse::<u64>())(input)
}

fn parse_stack(input: &str) -> IResult<&str, Stack> {
    let (input, addrs) = terminated(parse_stack_addrs, line_ending)(input)?;
    let (input, threads) = many1(terminated(preceded(space1, parse_thread), line_ending))(input)?;

    let stack = Stack {
        addrs,
        threads
    };

    Ok((input, stack))
}


fn parse_stack_addrs(input: &str) -> IResult<&str, Vec<i64>> {
    let (input, _) = tag("@")(input)?;
    many1(preceded(space1, hexadecimal_value))(input)
}

fn parse_mapped_library(input: &str) -> IResult<&str, MappedLibrary> {
    // 7f99f42dd000-7f99f42e0000
    let (input, first) = hexadecimal_value(input)?;
    let (input, _) = tag("-")(input)?;
    let (input, last) = hexadecimal_value(input)?;

    // r--p
    let (input, _) = preceded(space1, many_m_n(4, 4, anychar))(input)?;

    // 00000000
    let (input, _) = preceded(space1, many_m_n(8, 8, one_of("0123456789abcdefABCDEF")))(input)?;

    // 103:02
    let (input, _) = preceded(space1, digit1)(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, _) = digit1(input)?;

    // 5000
    let (input, _) = preceded(space1, digit1)(input)?;

    // /usr/lib/x86_64-linux-gnu/libgcc_s.so.1
    let (input, path) = preceded(space1, not_line_ending)(input)?;

    let library = MappedLibrary {
        first,
        last,
        path
    };

    Ok((input, library))
}

fn parse_thread(input: &str) -> IResult<&str, Thread> {
    let (input, _) = tag("t")(input)?;
    let (input, id) = take_while(|c: char| c.is_alphanumeric() || c == '*')(input)?;
    let (input, _) = tag(": ")(input)?;
    let (input, inuse_count) = map_res(digit1, |digit_str: &str| digit_str.parse::<u64>())(input)?;
    let (input, _) = tag(": ")(input)?;
    let (input, insuse_space) = map_res(digit1, |digit_str: &str| digit_str.parse::<u64>())(input)?;
    let (input, _) = tag(" [")(input)?;
    let (input, alloc_count) = map_res(digit1, |digit_str: &str| digit_str.parse::<u64>())(input)?;
    let (input, _) = tag(": ")(input)?;
    let (input, alloc_space) = map_res(digit1, |digit_str: &str| digit_str.parse::<u64>())(input)?;
    let (input, _) = tag("]")(input)?;

    let thread = Thread {
        id,
        inuse_count,
        insuse_space,
        alloc_count,
        alloc_space,
    };

    Ok((input, thread))
}

fn hexadecimal_value(input: &str) -> IResult<&str, i64> {
  map_res(
    preceded(
      opt(alt((tag("0x"), tag("0X")))),
      recognize(
        many1(
          terminated(one_of("0123456789abcdefABCDEF"), many0(char('_')))
        )
      )
    ),
    |out: &str| i64::from_str_radix(&str::replace(out, "_", ""), 16)
  ).parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_profile() {
        let data = "heap_v2/131072
  t*: 4385: 810327 [0: 0]
  t0: 129: 4965 [0: 0]
  t1: 191: 7942 [0: 0]
  t2: 0: 0 [0: 0]
  t3: 128: 5268 [0: 0]
  t4: 4: 405115494 [0: 0]
  t5: 468: 42015 [0: 0]
  t6: 148: 5586 [0: 0]
  t7: 232: 15290 [0: 0]
  t8: 0: 0 [0: 0]
@ 0x004 0x003 0x002 0x001
  t*: 1: 224 [0: 0]
  t5: 1: 224 [0: 0]
@ 0x001 0x002 0x003 0x004
  t*: 1: 224 [0: 0]
  t5: 1: 224 [0: 0]";
        let (_, profile) = parse_profile(data).unwrap();
        assert_eq!(131072, profile.sampling_rate);
        assert_eq!("*", profile.totals[0].id);
        assert_eq!(4385, profile.totals[0].inuse_count);
        assert_eq!(4385, profile.totals[0].inuse_count);
        assert_eq!(2, profile.stacks.len());

        let stack = &profile.stacks[0];
        assert_eq!(4, stack.addrs.len());
        assert_eq!(4, stack.addrs[0]);
    }

    #[test]
    fn test_parse_header() {
        let data = "heap_v2/12345";
        let (_, sampling_rate) = parse_header(data).unwrap();
        assert_eq!(12345, sampling_rate)
    }

    #[test]
    fn test_parse_stack_addrs() {
        let data = "@ 0x000000000001 0x000000000002 0x000000000003 0x000000000004";
        let (_, addrs) = parse_stack_addrs(data).unwrap();
        assert_eq!(4, addrs.len());
        assert_eq!(1, addrs[0]);
        assert_eq!(2, addrs[1]);
        assert_eq!(3, addrs[2]);
        assert_eq!(4, addrs[3]);
    }

    #[test]
    fn test_parse_library() {
        let data = "00000001-00000004 r--p 00000000 103:02 5000                      /usr/lib/x86_64-linux-gnu/libgcc_s.so.1";
        let (_, lib) = parse_mapped_library(data).unwrap();
        assert_eq!("/usr/lib/x86_64-linux-gnu/libgcc_s.so.1", lib.path);
        assert_eq!(1, lib.first);
        assert_eq!(4, lib.last);
    }

    #[test]
    fn test_parse_thread() {
        let data = "t123: 5000: 6000 [7000: 8000]";
        let result = parse_thread(data);

        match result {
            Ok((_, thread)) => {
                assert_eq!(thread.id, "123");
                assert_eq!(thread.inuse_count, 5000);
                assert_eq!(thread.insuse_space, 6000);
                assert_eq!(thread.alloc_count, 7000);
                assert_eq!(thread.alloc_space, 8000);
            }
            Err(err) => panic!("Parsing failed with error: {:?}", err),
        }
    }

    #[test]
    fn test_parse_thread_star() {
        let data = "t*: 5000: 6000 [7000: 9000]";
        let result = parse_thread(data);

        match result {
            Ok((_, thread)) => {
                assert_eq!(thread.id, "*");
                assert_eq!(thread.inuse_count, 5000);
                assert_eq!(thread.insuse_space, 6000);
                assert_eq!(thread.alloc_count, 7000);
                assert_eq!(thread.alloc_space, 9000);
            }
            Err(err) => panic!("Parsing failed with error: {:?}", err),
        }
    }
}
