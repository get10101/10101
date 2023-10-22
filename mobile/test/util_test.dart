import 'package:get_10101/features/wallet/application/util.dart';
import 'package:test/test.dart';

void main() {
  group('get formatted balance test', () {
    test('0.00 010 000', () {
      var (leading, balance) = getFormattedBalance(10000);
      expect(leading, "0.00 0");
      expect(balance, "10 000");
    });

    test('0.01 010 000', () {
      var (leading, balance) = getFormattedBalance(1010000);
      expect(leading, "0.0");
      expect(balance, "1 010 000");
    });

    test('0.11 010 000', () {
      var (leading, balance) = getFormattedBalance(11010000);
      expect(leading, "0.");
      expect(balance, "11 010 000");
    });

    test('2.11 010 000', () {
      var (leading, balance) = getFormattedBalance(211010000);
      expect(leading, "");
      expect(balance, "2.11 010 000");
    });
  });
}
