import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as rust;

enum OrderStatus {
  open,
  filled;

  static OrderStatus fromApi(rust.OrderStatus orderStatus) {
    switch (orderStatus) {
      case rust.OrderStatus.Open:
        return OrderStatus.open;
      case rust.OrderStatus.Filled:
        return OrderStatus.filled;
    }
  }
}

enum OrderType { market }

class Order {
  final Leverage leverage;
  final double quantity;
  final ContractSymbol contractSymbol;
  final Direction direction;
  final OrderStatus status;
  final OrderType type;

  Order(
      {required this.leverage,
      required this.quantity,
      required this.contractSymbol,
      required this.direction,
      required this.status,
      required this.type});
}
