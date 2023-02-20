import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/common/domain/model.dart';

enum PositionState {
  open,

  /// once the user pressed button to close position the button should be disabled otherwise the user can click it multiple times which would result in multiple orders and an open position in the other direction
  closing
}

class Position {
  final Leverage leverage;
  final double quantity;
  final ContractSymbol contractSymbol;
  final Direction direction;
  final double averageEntryPrice;
  final double liquidationPrice;
  final Amount unrealizedPnL;
  final PositionState positionState;

  Position(
      {required this.averageEntryPrice,
      required this.liquidationPrice,
      required this.leverage,
      required this.quantity,
      required this.contractSymbol,
      required this.direction,
      required this.positionState,
      required this.unrealizedPnL});
}
