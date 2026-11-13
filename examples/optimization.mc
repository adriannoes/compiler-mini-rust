/* Constants, a known branch, and unreachable statements. */
int main() {
  int value = (2 + 3) * 4;
  if (1 < 2) {
    value = value + (8 / 2);
  } else {
    value = 1 / 0;
  }
  (6 * 7);
  return value;
  value = 99;
}
