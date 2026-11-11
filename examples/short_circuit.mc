/* Logical operators skip unnecessary right operands. */
bool main() {
  bool ready = false;
  false && (ready = true);
  true || (1 / 0 == 0);
  return !ready;
}
