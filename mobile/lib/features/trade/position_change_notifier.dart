import 'dart:collection';
import 'dart:developer';
import 'package:flutter/material.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/features/trade/application/position_service.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

import 'domain/position.dart';

class PositionChangeNotifier extends ChangeNotifier implements Subscriber {
  HashMap<ContractSymbol, Position> positions = HashMap();

  Future<void> _create(PositionService positionService) async {
    List<Position> positions = await positionService.fetchPositions();
    for (Position position in positions) {
      this.positions[position.contractSymbol] = position;
    }

    notifyListeners();
  }

  PositionChangeNotifier.create(PositionService positionService) {
    _create(positionService);
  }

  @override
  void notify(bridge.Event event) {
    log("Receiving this in the position notifier: ${event.toString()}");

    if (event is bridge.Event_PositionUpdateNotification) {
      Position position = Position.fromApi(event.field0);
      positions[position.contractSymbol] = position;
    } else {
      log("Received unexpected event: ${event.toString()}");
    }

    notifyListeners();
  }
}
