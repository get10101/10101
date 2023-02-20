import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';

import 'domain/position.dart';

class PositionChangeNotifier extends ChangeNotifier {
  // TODO: Remove dummy position once we actually handle updates from the backend
  final Position _dummy = Position(
      averageEntryPrice: 19000,
      liquidationPrice: 14000,
      leverage: Leverage(2),
      quantity: 100,
      contractSymbol: ContractSymbol.btcusd,
      direction: Direction.long,
      unrealizedPnL: Amount(-400),
      positionState: PositionState.open);

  Position? position;

  PositionChangeNotifier() {
    position = _dummy;
  }

  updatePosition(Position position) async {
    this.position = position;
    notifyListeners();
  }
}
