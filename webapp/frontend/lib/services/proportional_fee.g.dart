// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'proportional_fee.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

ProportionalFee _$ProportionalFeeFromJson(Map<String, dynamic> json) => ProportionalFee(
      percentage: (json['percentage'] as num).toDouble(),
      minSats: json['min_sats'] as int? ?? 0,
    );

Map<String, dynamic> _$ProportionalFeeToJson(ProportionalFee instance) => <String, dynamic>{
      'percentage': instance.percentage,
      'min_sats': instance.minSats,
    };
