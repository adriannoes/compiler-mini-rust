/* Type-safe calls, conditions, and returns. */
int choose(bool use_first, int first, int second) {
  if (use_first) {
    return first;
  } else {
    return second;
  }
}
void consume(int value) {
  bool positive = value > 0;
  if (positive) {
    value = value - 1;
  }
}
int main() {
  int result = choose(true, 2, 3);
  consume(result);
  return result;
}
