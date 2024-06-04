import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/domain/funding_rate.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';

class FundingRateChangeNotifier extends ChangeNotifier implements Subscriber {
  FundingRate? nextRate;

  Future<void> initialize() async {
    notifyListeners();
  }

  FundingRateChangeNotifier();

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_NextFundingRate) {
      nextRate = FundingRate.fromApi(event.field0);

      notifyListeners();
    } else {
      logger.w("Received unexpected event: ${event.toString()}");
    }
  }
}
