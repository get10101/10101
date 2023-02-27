import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

enum PositionState {
  open,

  /// once the user pressed button to close position the button should be disabled otherwise the user can click it multiple times which would result in multiple orders and an open position in the other direction
  closing;

  static PositionState fromApi(bridge.PositionState positionState) {
    switch (positionState) {
      case bridge.PositionState.Open:
        return PositionState.open;
      case bridge.PositionState.Closing:
        return PositionState.closing;
    }
  }
}

class Position {
  final Leverage leverage;
  final double quantity;
  final ContractSymbol contractSymbol;
  final Direction direction;
  final double averageEntryPrice;
  final double liquidationPrice;
  final Amount unrealizedPnl;
  final PositionState positionState;
  final Amount collateral;

  Position(
      {required this.averageEntryPrice,
      required this.liquidationPrice,
      required this.leverage,
      required this.quantity,
      required this.contractSymbol,
      required this.direction,
      required this.positionState,
      required this.unrealizedPnl,
      required this.collateral});

  static Position fromApi(bridge.Position position) {
    return Position(
        leverage: Leverage(position.leverage),
        quantity: position.quantity,
        contractSymbol: ContractSymbol.fromApi(position.contractSymbol),
        direction: Direction.fromApi(position.direction),
        positionState: PositionState.fromApi(position.positionState),
        averageEntryPrice: position.averageEntryPrice,
        liquidationPrice: position.liquidationPrice,
        unrealizedPnl: Amount(position.unrealizedPnl),
        collateral: Amount(position.collateral));
  }

  static bridge.Position apiDummy() {
    return bridge.Position(
      leverage: 0,
      quantity: 0,
      contractSymbol: bridge.ContractSymbol.BtcUsd,
      direction: bridge.Direction.Long,
      positionState: bridge.PositionState.Open,
      averageEntryPrice: 0,
      liquidationPrice: 0,
      unrealizedPnl: 0,
      collateral: 0,
    );
  }
}
