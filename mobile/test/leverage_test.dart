import 'package:flutter_test/flutter_test.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';

void main() {
  test('Test leverage equality', () {
    final leverage1 = Leverage(1);
    final leverageAlso1 = Leverage(1);

    expect(leverage1, equals(leverageAlso1));
  });

  test('Test leverage not equality', () {
    final leverage1 = Leverage(1);
    final leverage2 = Leverage(2);

    expect(leverage1, isNot(equals(leverage2)));
  });
}
