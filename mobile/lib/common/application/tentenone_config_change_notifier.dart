import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/liquidity_option.dart';
import 'package:get_10101/common/domain/referral_status.dart';

class TenTenOneConfigChangeNotifier extends ChangeNotifier implements Subscriber {
  ChannelInfoService channelInfoService;

  List<LiquidityOption> _liquidityOptions = [];
  ReferralStatus? _referralStatus;
  // will be overwritten once we receive the authenticated event
  int _maxLeverage = 5;

  TenTenOneConfigChangeNotifier(this.channelInfoService);

  List<LiquidityOption> getLiquidityOptions(bool activeOnly) {
    return _liquidityOptions.where((option) => option.active || !activeOnly).toList();
  }

  ReferralStatus? get referralStatus => _referralStatus;

  double get maxLeverage => _maxLeverage.toDouble();

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_Authenticated) {
      _liquidityOptions =
          event.field0.liquidityOptions.map((lo) => LiquidityOption.from(lo)).toList();
      _liquidityOptions.sort((a, b) => a.rank.compareTo(b.rank));

      _referralStatus = ReferralStatus.from(event.field0.referralStatus);
      _maxLeverage = event.field0.maxLeverage;

      super.notifyListeners();
    }
  }
}
