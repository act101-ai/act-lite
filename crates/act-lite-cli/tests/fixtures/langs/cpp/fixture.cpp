#include <string>

namespace util {
int twice(int v) { return v * 2; }
}

class Counter {
public:
  int next() { return n++; }
private:
  int n = 0;
};

int main() { return util::twice(2); }
