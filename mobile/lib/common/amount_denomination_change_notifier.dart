import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/model.dart';

class AmountDenominationChangeNotifier extends ChangeNotifier {
  AmountDenomination denomination = AmountDenomination.satoshi;

  void updateDenomination(AmountDenomination denomination) {
    this.denomination = denomination;
    notifyListeners();
  }
}
