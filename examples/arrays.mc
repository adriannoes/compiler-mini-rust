/* Local arrays with scalar arguments and checked indices. */
int next(int value) {
  return value + 1;
}

int main() {
  int values[4];
  bool flags[2];
  int index = 0;
  int total = 0;
  while (index < 4) {
    values[index] = next(index);
    total = total + values[index];
    index = index + 1;
  }
  flags[0] = false;
  flags[1] = total == 10;
  if (!flags[0] && flags[1]) return total;
  else return 0;
}
