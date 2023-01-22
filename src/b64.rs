
static TABLE: [char; 64] =
['0','1','2','3', '4','5','6','7', '8','9','A','B', 'C','D','E','F',
 'G','H','I','J', 'K','L','M','N', 'O','P','Q','R', 'S','T','U','V',
 'W','X','Y','Z', 'a','b','c','d', 'e','f','g','h', 'i','j','k','l',
 'm','n','o','p', 'q','r','s','t', 'u','v','w','x', 'y','z','+','-'];

pub fn from_u64(num: u64) -> String {
    let mut out = String::new();

    let mut num = num;

    let mut i = 64;

    if num == 0 {
        out = "0".to_owned();
    }

    while num > 0 {
        let j = num % i;

        out.push(TABLE[j as usize]);

        num /= i;
        i *= 64;
    }

    out.chars().rev().collect::<String>()
}

pub fn to_u64(string: &str) -> u64 {
    let mut pow = 1;
    let mut out: u64 = 0;

    for digit in string.trim().chars().rev() {
        let digit_val = TABLE.iter().position(|x| x==&digit).unwrap();

        out += (digit_val as u64) * pow;

        pow *= 64;
    }

    out
}
