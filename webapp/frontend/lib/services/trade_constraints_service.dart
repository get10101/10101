import 'dart:convert';

import 'package:flutter/widgets.dart';
import 'package:get_10101/common/http_client.dart';
import 'package:json_annotation/json_annotation.dart';

part 'trade_constraints_service.g.dart';

class TradeConstraintsService {
  const TradeConstraintsService();

  Future<TradeConstraints> getTradeConstraints() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/tradeconstraints'));

    if (response.statusCode == 200) {
      final jsonData = jsonDecode(response.body);
      return TradeConstraints.fromJson(jsonData);
    } else {
      throw FlutterError("Failed to fetch liquidity options phrase");
    }
  }
}

@JsonSerializable()
class TradeConstraints {
  /// Max balance the local party can use
  ///
  /// This depends on whether the user has a channel or not. If he has a channel, then his
  /// channel balance is the max amount, otherwise his on-chain balance dictates the max amount
  @JsonKey(name: 'max_local_balance_sats')
  final int maxLocalBalanceSats;

  /// Max amount the counterparty is willing to put.
  ///
  /// This depends whether the user has a channel or not, i.e. if he has a channel then the max
  /// amount is what the counterparty has in the channel, otherwise, it's a fixed amount what
  /// the counterparty is willing to provide.
  @JsonKey(name: 'max_counterparty_balance_sats')
  final int maxCounterpartyBalanceSats;

  /// The leverage the coordinator will take
  @JsonKey(name: 'coordinator_leverage')
  final double coordinatorLeverage;

  /// Smallest allowed amount of contracts
  @JsonKey(name: 'min_quantity')
  final int minQuantity;

  /// If true it means that the user has a channel and hence the max amount is limited by what he
  /// has in the channel. In the future we can consider splice in and allow the user to use more
  /// than just his channel balance.
  @JsonKey(name: 'is_channel_balance')
  final bool isChannelBalance;

  /// Smallest allowed margin
  @JsonKey(name: 'min_margin_sats')
  final int minMarginSats;

  /// The estimated fee to be paid to open a channel in sats
  @JsonKey(name: 'estimated_funding_tx_fee_sats')
  final int estimatedFundingTxFeeSats;

  /// The fee we need to reserve in the channel reserve for tx fees
  @JsonKey(name: 'channel_fee_reserve_sats')
  final int channelFeeReserveSats;

  const TradeConstraints({
    required this.maxLocalBalanceSats,
    required this.maxCounterpartyBalanceSats,
    required this.coordinatorLeverage,
    required this.minQuantity,
    required this.isChannelBalance,
    required this.minMarginSats,
    required this.estimatedFundingTxFeeSats,
    required this.channelFeeReserveSats,
  });

  factory TradeConstraints.fromJson(Map<String, dynamic> json) => _$TradeConstraintsFromJson(json);

  Map<String, dynamic> toJson() => _$TradeConstraintsToJson(this);
}
