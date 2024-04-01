// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'trade_constraints_service.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

TradeConstraints _$TradeConstraintsFromJson(Map<String, dynamic> json) => TradeConstraints(
      maxLocalMarginSats: json['max_local_margin_sats'] as int,
      maxCounterpartyMarginSats: json['max_counterparty_margin_sats'] as int,
      coordinatorLeverage: (json['coordinator_leverage'] as num).toDouble(),
      minQuantity: json['min_quantity'] as int,
      isChannelBalance: json['is_channel_balance'] as bool,
      minMarginSats: json['min_margin_sats'] as int,
      estimatedFundingTxFeeSats: json['estimated_funding_tx_fee_sats'] as int,
      channelFeeReserveSats: json['channel_fee_reserve_sats'] as int,
    );

Map<String, dynamic> _$TradeConstraintsToJson(TradeConstraints instance) => <String, dynamic>{
      'max_local_margin_sats': instance.maxLocalMarginSats,
      'max_counterparty_margin_sats': instance.maxCounterpartyMarginSats,
      'coordinator_leverage': instance.coordinatorLeverage,
      'min_quantity': instance.minQuantity,
      'is_channel_balance': instance.isChannelBalance,
      'min_margin_sats': instance.minMarginSats,
      'estimated_funding_tx_fee_sats': instance.estimatedFundingTxFeeSats,
      'channel_fee_reserve_sats': instance.channelFeeReserveSats,
    };
