import 'package:get_10101/common/model.dart';
import 'package:json_annotation/json_annotation.dart';

part 'proportional_fee.g.dart';

/// A representation of a proportional fee of an amount with an optional min amount.
@JsonSerializable()
class ProportionalFee {
  double percentage;
  @JsonKey(name: 'min_sats')
  int minSats;

  ProportionalFee({required this.percentage, this.minSats = 0});

  Amount getFee(Amount amount) {
    final fee = (amount.sats / 100) * percentage;
    return fee < minSats ? Amount(minSats) : Amount(fee.ceil());
  }

  factory ProportionalFee.fromJson(Map<String, dynamic> json) => _$ProportionalFeeFromJson(json);
  Map<String, dynamic> toJson() => _$ProportionalFeeToJson(this);
}
