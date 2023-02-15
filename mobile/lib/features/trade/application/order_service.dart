import 'package:get_10101/ffi.dart' as rust;

import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';

class OrderService {
  Future<void> submitMarketOrder(Leverage leverage, double quantity, ContractSymbol contractSymbol,
      Direction direction) async {
    rust.NewOrder order = rust.NewOrder(
        leverage: leverage.leverage,
        quantity: quantity,
        contractSymbol: contractSymbol.toApi(),
        direction: direction.toApi(),
        orderType: const rust.OrderType.market());

    await rust.api.submitOrder(order: order);
  }
}
