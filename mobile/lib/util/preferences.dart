import 'package:flutter/foundation.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:shared_preferences/shared_preferences.dart';

class Preferences {
  Preferences._privateConstructor();

  static final Preferences instance = Preferences._privateConstructor();

  static const contactDetails = "emailAddress";
  static const openPosition = "openPosition";
  static const fullBackup = "fullBackup";
  static const logLevelTrace = "logLevelTrace";
  static const _hasSeenReferralDialogTimePassed = "hasSeenReferralDialogTimePassed";

  Future<bool> setLogLevelTrace(bool trace) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.setBool(logLevelTrace, trace);
  }

  Future<bool> isLogLevelTrace() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getBool(logLevelTrace) ?? kDebugMode;
  }

  Future<bool> setFullBackupRequired(bool required) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.setBool(fullBackup, required);
  }

  Future<bool> isFullBackupRequired() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getBool(fullBackup) ?? true;
  }

  getOpenPosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getString(openPosition);
  }

  setOpenStablePosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(openPosition, WalletScreen.label);
  }

  setOpenTradePosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(openPosition, TradeScreen.label);
  }

  unsetOpenPosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.remove(openPosition);
  }

  Future<bool> setContactDetails(String value) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.setString(contactDetails, value);
  }

  Future<String> getContactDetails() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getString(contactDetails) ?? "";
  }

  Future<bool> hasReferralDialogTimePassedMoreThan7days() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    int? storedTimestamp = preferences.getInt(_hasSeenReferralDialogTimePassed);

    if (storedTimestamp != null) {
      // Calculate the difference to today
      DateTime now = DateTime.now();
      int differenceInDays =
          now.difference(DateTime.fromMillisecondsSinceEpoch(storedTimestamp)).inDays;
      return differenceInDays > 7;
    } else {
      // If no timestamp is stored, it is considered older than 7 days
      return true;
    }
  }

  Future<void> storeDontShowReferralDialogFor7Days() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setInt(_hasSeenReferralDialogTimePassed, DateTime.now().millisecondsSinceEpoch);
  }

  Future<bool> hasContactDetails() async {
    var contact = await getContactDetails();
    return contact.isNotEmpty;
  }
}
