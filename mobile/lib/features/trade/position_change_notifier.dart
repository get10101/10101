import 'dart:developer';
import 'package:flutter/material.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/position_service.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

import 'domain/position.dart';

class PositionChangeNotifier extends ChangeNotifier implements Subscriber {
  late PositionService _positionService;
  Map<ContractSymbol, Position> positions = {};

  late double _bid;
  late double _ask;

  Future<void> initialize() async {
    List<Position> positions = await _positionService.fetchPositions();
    for (Position position in positions) {
      this.positions[position.contractSymbol] = position;
    }

    // TODO: fetch price from backend and wire in price updates
    _bid = 23000;
    _ask = 23100;

    notifyListeners();
  }

  PositionChangeNotifier(PositionService positionService) {
    _positionService = positionService;
  }

  @override
  void notify(bridge.Event event) {
    log("Receiving this in the position notifier: ${event.toString()}");

    if (event is bridge.Event_PositionUpdateNotification) {
      Position position = Position.fromApi(event.field0);

      position.unrealizedPnl = Amount(_positionService.calculatePnl(position, _bid, _ask));

      positions[position.contractSymbol] = position;
    } else {
      log("Received unexpected event: ${event.toString()}");
    }

    notifyListeners();
  }
}
