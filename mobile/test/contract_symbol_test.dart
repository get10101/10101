import 'package:flutter_test/flutter_test.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';

void main() {
  test('Contract symbol label correct', () {
    const contractSymbol = ContractSymbol.btcusd;
    expect(contractSymbol.label, "BTC/USD");
  });
}
