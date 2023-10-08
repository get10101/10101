import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';

enum PositionState {
  open,

  /// once the user pressed button to close position the button should be disabled otherwise the user can click it multiple times which would result in multiple orders and an open position in the other direction
  closing,
  rollover;

  static PositionState fromApi(bridge.PositionState positionState) {
    switch (positionState) {
      case bridge.PositionState.Open:
        return PositionState.open;
      case bridge.PositionState.Closing:
        return PositionState.closing;
      case bridge.PositionState.Rollover:
        return PositionState.rollover;
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

  // The unrealized PnL is calculated from the current price
  Amount? unrealizedPnl;
  final PositionState positionState;
  final Amount collateral;
  final DateTime expiry;

  Position(
      {required this.averageEntryPrice,
      required this.liquidationPrice,
      required this.leverage,
      required this.quantity,
      required this.contractSymbol,
      required this.direction,
      required this.positionState,
      this.unrealizedPnl,
      required this.collateral,
      required this.expiry});

  bool isStable() => direction == Direction.short && leverage == Leverage(1);

  Amount getAmountWithUnrealizedPnl() {
    if (unrealizedPnl != null) {
      return Amount(collateral.sats + unrealizedPnl!.sats);
    }

    return collateral;
  }

  static Position fromApi(bridge.Position position) {
    return Position(
      leverage: Leverage(position.leverage),
      quantity: position.quantity,
      contractSymbol: ContractSymbol.fromApi(position.contractSymbol),
      direction: Direction.fromApi(position.direction),
      positionState: PositionState.fromApi(position.positionState),
      averageEntryPrice: position.averageEntryPrice,
      liquidationPrice: position.liquidationPrice,
      collateral: Amount(position.collateral),
      expiry: DateTime.fromMillisecondsSinceEpoch(position.expiry * 1000),
    );
  }

  static bridge.Position apiDummy() {
    return const bridge.Position(
      leverage: 0,
      quantity: 0,
      contractSymbol: bridge.ContractSymbol.BtcUsd,
      direction: bridge.Direction.Long,
      positionState: bridge.PositionState.Open,
      averageEntryPrice: 0,
      liquidationPrice: 0,
      collateral: 0,
      expiry: 0,
    );
  }
}
