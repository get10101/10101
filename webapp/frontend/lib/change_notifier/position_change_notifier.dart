import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/services/position_service.dart';

class PositionChangeNotifier extends ChangeNotifier {
  final PositionService service;
  late Timer timer;

  List<Position>? _positions;

  PositionChangeNotifier(this.service) {
    _refresh();
    Timer.periodic(const Duration(seconds: 2), (timer) async {
      _refresh();
    });
  }

  void _refresh() async {
    try {
      final positions = await service.fetchOpenPositions();
      _positions = positions;

      super.notifyListeners();
    } catch (error) {
      logger.e(error);
    }
  }

  List<Position>? getPositions() => _positions;

  @override
  void dispose() {
    super.dispose();
    timer.cancel();
  }
}
