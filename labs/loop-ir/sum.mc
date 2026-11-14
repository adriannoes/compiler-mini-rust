int next(int value) {
  return value + 1;
}

int sum_next(int limit) {
  int index = 0;
  int total = 0;
  while (index < limit) {
    total = total + next(index);
    index = index + 1;
  }
  return total;
}
